use core::pin::Pin;
use std::task::Poll;

use pin_project::pin_project;

use crate::source::{source_poll::SourcePollOk, traits::SourceContext, Source, SourcePoll};

#[pin_project]
pub struct Shift<Src: Source, T: Ord + Copy> {
    #[pin]
    source: Src,

    into_new: fn(Src::Time) -> T,
    into_old: fn(T) -> Src::Time,
}

impl<Src: Source, T: Ord + Copy> Shift<Src, T> {
    pub fn new(source: Src, into_new: fn(Src::Time) -> T, into_old: fn(T) -> Src::Time) -> Self {
        Self {
            source,
            into_new,
            into_old,
        }
    }

    fn poll_internal<F, S>(
        mut self: Pin<&mut Self>,
        time: T,
        cx: SourceContext<'_, '_>,
        poll_fn: F,
    ) -> SourcePoll<T, Src::Event, S, Src::Error>
    where
        F: Fn(
            Pin<&mut Src>,
            Src::Time,
            SourceContext<'_, '_>,
        ) -> SourcePoll<Src::Time, Src::Event, S, Src::Error>,
    {
        let proj = self.project();
        let source = proj.source;
        let into_new = proj.into_new;
        let into_old = proj.into_old;

        match poll_fn(source, into_old(time), cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
            Poll::Ready(Ok(result)) => Poll::Ready(Ok(match result {
                SourcePollOk::Rollback(t) => SourcePollOk::Rollback(into_new(t)),
                SourcePollOk::Event(e, t) => SourcePollOk::Event(e, into_new(t)),
                SourcePollOk::Scheduled(s, t) => SourcePollOk::Scheduled(s, into_new(t)),
                SourcePollOk::Ready(s) => SourcePollOk::Ready(s),
            })),
        }
    }
}

impl<Src: Source, T: Ord + Copy> Source for Shift<Src, T> {
    type Time = T;

    type Event = Src::Event;

    type State = Src::State;

    type Error = Src::Error;

    fn poll(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: SourceContext<'_, '_>,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, Self::State, Src::Error> {
        self.poll_internal(time, cx, Src::poll)
    }

    fn poll_forget(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: SourceContext<'_, '_>,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, Self::State, Src::Error> {
        self.poll_internal(time, cx, Src::poll_forget)
    }

    fn poll_events(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: SourceContext<'_, '_>,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, (), Src::Error> {
        self.poll_internal(time, cx, Src::poll_events)
    }

    fn max_channels(&self) -> std::num::NonZeroUsize {
        self.source.max_channels()
    }
}
