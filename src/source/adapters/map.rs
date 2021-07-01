use core::pin::Pin;
use core::task::Context;
use pin_project::pin_project;

use crate::source::Source;
use crate::source::SourcePoll;

#[pin_project]
pub struct Map<Src: Source, E, S> {
    #[pin]
    source: Src,

    event_transform: fn(Src::Event) -> E,
    state_transform: fn(Src::State) -> S,
}

impl<Src: Source, E, S> Map<Src, E, S> {
    pub fn new(
        source: Src,
        event_transform: fn(Src::Event) -> E,
        state_transform: fn(Src::State) -> S,
    ) -> Map<Src, E, S> {
        Self {
            source,
            event_transform,
            state_transform,
        }
    }
}

impl<Src: Source, E, S> Source for Map<Src, E, S> {
    type Time = Src::Time;
    type Event = E;
    type State = S;

    fn poll(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: &mut Context<'_>,
    ) -> SourcePoll<Src::Time, E, S> {
        let this = self.project();
        let e_fn = this.event_transform;
        let s_fn = this.state_transform;

        match this.source.poll(time, cx) {
            SourcePoll::Pending => SourcePoll::Pending,
            SourcePoll::Rollback(t) => SourcePoll::Rollback(t),
            SourcePoll::Event(e, t) => SourcePoll::Event(e_fn(e), t),
            SourcePoll::Scheduled(s, t) => SourcePoll::Scheduled(s_fn(s), t),
            SourcePoll::Ready(s) => SourcePoll::Ready(s_fn(s)),
        }
    }
}
