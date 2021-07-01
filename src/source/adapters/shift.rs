use core::pin::Pin;
use core::task::Context;

use pin_project::pin_project;

use crate::source::{Source, SourcePoll};

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
}

impl<Src: Source, T: Ord + Copy> Source for Shift<Src, T> {
    type Time = T;

    type Event = Src::Event;

    type State = Src::State;

    fn poll(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: &mut Context<'_>,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, Self::State> {
        let proj = self.project();
        let source = proj.source;
        let into_new = proj.into_new;
        let into_old = proj.into_old;

        match source.poll(into_old(time), cx) {
            SourcePoll::Pending => SourcePoll::Pending,
            SourcePoll::Rollback(t) => SourcePoll::Rollback(into_new(t)),
            SourcePoll::Event(e, t) => SourcePoll::Event(e, into_new(t)),
            SourcePoll::Scheduled(s, t) => SourcePoll::Scheduled(s, into_new(t)),
            SourcePoll::Ready(s) => SourcePoll::Ready(s),
        }
    }

    fn poll_forget(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: &mut Context<'_>,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, Self::State> {
        let proj = self.project();
        let source = proj.source;
        let into_new = proj.into_new;
        let into_old = proj.into_old;

        match source.poll_forget(into_old(time), cx) {
            SourcePoll::Pending => SourcePoll::Pending,
            SourcePoll::Rollback(t) => SourcePoll::Rollback(into_new(t)),
            SourcePoll::Event(e, t) => SourcePoll::Event(e, into_new(t)),
            SourcePoll::Scheduled(s, t) => SourcePoll::Scheduled(s, into_new(t)),
            SourcePoll::Ready(s) => SourcePoll::Ready(s),
        }
    }

    fn poll_events(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: &mut Context<'_>,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, ()> {
        let proj = self.project();
        let source = proj.source;
        let into_new = proj.into_new;
        let into_old = proj.into_old;

        match source.poll_events(into_old(time), cx) {
            SourcePoll::Pending => SourcePoll::Pending,
            SourcePoll::Rollback(t) => SourcePoll::Rollback(into_new(t)),
            SourcePoll::Event(e, t) => SourcePoll::Event(e, into_new(t)),
            SourcePoll::Scheduled((), t) => SourcePoll::Scheduled((), into_new(t)),
            SourcePoll::Ready(()) => SourcePoll::Ready(()),
        }
    }
}
