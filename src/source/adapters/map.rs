use core::pin::Pin;
use core::task::Context;
use pin_project::pin_project;

use crate::source::Source;
use crate::source::SourcePoll;

#[pin_project]
pub struct Map<St: Source, E, S> {
    #[pin]
    stream: St,

    event_transform: fn(St::Event) -> E,
    state_transform: fn(St::State) -> S,
}

impl<St: Source, E, S> Map<St, E, S> {
    pub fn new(
        stream: St,
        event_transform: fn(St::Event) -> E,
        state_transform: fn(St::State) -> S,
    ) -> Map<St, E, S> {
        Self {
            stream,
            event_transform,
            state_transform,
        }
    }
}

impl<St: Source, E, S> Source for Map<St, E, S> {
    type Time = St::Time;
    type Event = E;
    type State = S;

    fn poll(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: &mut Context<'_>,
    ) -> SourcePoll<St::Time, E, S> {
        let this = self.project();
        let e_fn = this.event_transform;
        let s_fn = this.state_transform;

        match this.stream.poll(time, cx) {
            SourcePoll::Pending => SourcePoll::Pending,
            SourcePoll::Rollback(t) => SourcePoll::Rollback(t),
            SourcePoll::Event(e, t) => SourcePoll::Event(e_fn(e), t),
            SourcePoll::Scheduled(s, t) => SourcePoll::Scheduled(s_fn(s), t),
            SourcePoll::Ready(s) => SourcePoll::Ready(s_fn(s)),
        }
    }
}
