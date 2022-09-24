use std::num::NonZeroUsize;
use std::pin::Pin;
use std::task::Waker;

use crate::source_poll::TrySourcePoll;
use crate::traits::{ConcurrentSource, SourceContext};
use crate::Source;

pub struct MutexSource<Src: Source>(parking_lot::Mutex<Src>);

impl<Src: Source> MutexSource<Src> {
    pub fn new(source: Src) -> Self {
        Self(parking_lot::Mutex::new(source))
    }
}

impl<Src: Source> Source for MutexSource<Src> {
    type Time = Src::Time;

    type Event = Src::Event;

    type State = Src::State;

    type Error = Src::Error;

    fn poll(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: SourceContext,
    ) -> TrySourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
        self.poll_concurrent(time, cx)
    }

    fn poll_forget(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: SourceContext,
    ) -> TrySourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
        self.poll_forget_concurrent(time, cx)
    }

    fn poll_events(
        self: Pin<&mut Self>,
        time: Self::Time,
        all_channel_waker: Waker,
    ) -> TrySourcePoll<Self::Time, Self::Event, (), Self::Error> {
        self.poll_events_concurrent(time, all_channel_waker)
    }

    fn release_channel(self: Pin<&mut Self>, channel: usize) {
        self.release_channel_concurrent(channel)
    }

    fn advance(self: Pin<&mut Self>, time: Self::Time) {
        self.advance_concurrent(time)
    }

    fn max_channel(&self) -> NonZeroUsize {
        self.0.lock().max_channel()
    }
}

impl<Src: Source> ConcurrentSource for MutexSource<Src> {
    fn poll_concurrent(
        &self,
        time: Self::Time,
        cx: SourceContext,
    ) -> TrySourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
        let mut source = self.0.lock();
        unsafe { Pin::new_unchecked(&mut *source) }.poll(time, cx)
    }

    fn poll_forget_concurrent(
        &self,
        time: Self::Time,
        cx: SourceContext,
    ) -> TrySourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
        let mut source = self.0.lock();
        unsafe { Pin::new_unchecked(&mut *source) }.poll_forget(time, cx)
    }

    fn poll_events_concurrent(
        &self,
        time: Self::Time,
        all_channel_waker: Waker,
    ) -> TrySourcePoll<Self::Time, Self::Event, (), Self::Error> {
        let mut source = self.0.lock();
        unsafe { Pin::new_unchecked(&mut *source) }.poll_events(time, all_channel_waker)
    }

    fn release_channel_concurrent(&self, channel: usize) {
        let mut source = self.0.lock();
        unsafe { Pin::new_unchecked(&mut *source) }.release_channel(channel)
    }

    fn advance_concurrent(&self, time: Self::Time) {
        let mut source = self.0.lock();
        unsafe { Pin::new_unchecked(&mut *source) }.advance(time)
    }
}
