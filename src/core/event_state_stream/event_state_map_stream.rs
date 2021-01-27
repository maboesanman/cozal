use core::task::Context;
use core::pin::Pin;
use pin_project::pin_project;

use super::{EventStatePoll, EventStateStream};


#[pin_project]
struct EventStateMapStream<St: EventStateStream, E, S> 
{
    #[pin]
    stream: St,

    event_transform: fn(St::Event) -> E,
    state_transform: fn(St::State) -> S,
}

impl<St: EventStateStream, E, S> EventStateMapStream<St, E, S> {
    fn new(
        stream: St,
        event_transform: fn(St::Event) -> E,
        state_transform: fn(St::State) -> S,
    ) -> EventStateMapStream<St, E, S> {
        Self {
            stream,
            event_transform,
            state_transform,
        }
    }
}

impl<St: EventStateStream, E, S> EventStateStream for EventStateMapStream<St, E, S> {
    type Time = St::Time;
    type Event = E;
    type State = S;

    fn poll(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: &mut Context<'_>,
    ) -> EventStatePoll<St::Time, E, S> {
        let this = self.project();
        let e_fn = this.event_transform;
        let s_fn = this.state_transform;

        match this.stream.poll(time, cx) {
            EventStatePoll::Pending => EventStatePoll::Pending,
            EventStatePoll::Rollback(t) => EventStatePoll::Rollback(t),
            EventStatePoll::Event(t, e) => EventStatePoll::Event(t, e_fn(e)),
            EventStatePoll::Scheduled(t, s) => EventStatePoll::Scheduled(t, s_fn(s)),
            EventStatePoll::Ready(s) => EventStatePoll::Ready(s_fn(s)),
            EventStatePoll::Done(s) => EventStatePoll::Done(s_fn(s)),
        }
    }
}
