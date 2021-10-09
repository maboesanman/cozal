use core::pin::Pin;
use std::num::NonZeroUsize;
use std::sync::Arc;

mod advanced;
mod channel_map;
mod duplicate_inner;
mod duplicate_waker;
mod original;
mod rollback_event;

use self::duplicate_inner::DuplicateInner;
use crate::source::traits::SourceContext;
use crate::source::{Source, SourcePoll};

pub struct Duplicate<Src: Source>
where
    Src::Event: Clone,
{
    inner: Arc<DuplicateInner<Src>>,
}

impl<Src: Source> Duplicate<Src>
where
    Src::Event: Clone,
{
    pub fn new(source: Src) -> Self {
        Self {
            inner: DuplicateInner::new(source),
        }
    }
}

impl<Src: Source> Clone for Duplicate<Src>
where
    Src::Event: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone_inner(),
        }
    }
}

impl<Src: Source> Source for Duplicate<Src>
where
    Src::Event: Clone,
{
    type Time = Src::Time;

    type Event = Src::Event;

    type State = Src::State;

    type Error = Src::Error;

    fn poll(
        self: Pin<&mut Self>,
        poll_time: Self::Time,
        cx: SourceContext,
    ) -> SourcePoll<Self::Time, Self::Event, Self::State, Src::Error> {
        self.inner.poll(poll_time, cx, Src::poll)
    }

    fn poll_forget(
        self: Pin<&mut Self>,
        poll_time: Self::Time,
        cx: SourceContext,
    ) -> SourcePoll<Self::Time, Self::Event, Self::State, Src::Error> {
        self.inner.poll(poll_time, cx, Src::poll_forget)
    }

    fn poll_events(
        self: Pin<&mut Self>,
        poll_time: Self::Time,
        cx: SourceContext,
    ) -> SourcePoll<Self::Time, Self::Event, (), Src::Error> {
        self.inner.poll(poll_time, cx, Src::poll_events)
    }

    fn advance(self: Pin<&mut Self>, time: Src::Time) {
        self.inner.advance(time)
    }

    fn max_channel(&self) -> NonZeroUsize {
        self.inner.max_channel()
    }
}

pub type PollFn<Src, State> =
    fn(
        Pin<&mut Src>,
        <Src as Source>::Time,
        SourceContext,
    )
        -> SourcePoll<<Src as Source>::Time, <Src as Source>::Event, State, <Src as Source>::Error>;
