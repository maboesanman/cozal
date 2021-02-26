use flume::{unbounded, Receiver, Sender};
use futures::Stream;
use im::OrdMap;
use pin_project::pin_project;
use std::{pin::Pin, task::Poll};

use crate::core::event_state_stream::{EventStatePoll, EventStateStream};


#[pin_project]
pub struct TestStatefulStream<Time: Ord + Copy, Event: Clone, State: Clone> {
    #[pin]
    receiver: Receiver<(Time, Event)>,

    events: OrdMap<Time, Event>,
    current_state: State,
    done: bool,
    working: bool,
}

impl<Time: Ord + Copy, Event: Clone, State: Clone> TestStatefulStream<Time, Event, State> {
    pub fn new(current_state: State) -> (Sender<(Time, Event)>, Self) {
        let (sender, receiver) = unbounded();
        (
            sender,
            Self {
                receiver,
                events: OrdMap::new(),
                current_state,
                done: false,
                working: false,
            },
        )
    }

    pub fn set_state(self: Pin<&mut Self>, new_state: State) {
        *self.project().current_state = new_state;
    }

    pub fn set_working(self: Pin<&mut Self>, working: bool) {
        *self.project().working = working;
    }
}

impl<Time: Ord + Copy, Event: Clone, State: Clone> EventStateStream
    for TestStatefulStream<Time, Event, State>
{
    type Time = Time;

    type Event = Event;

    type State = State;

    fn poll(
        self: Pin<&mut Self>,
        poll_time: Self::Time,
        cx: &mut std::task::Context<'_>,
    ) -> EventStatePoll<Self::Time, Self::Event, Self::State>
    {
        let mut proj = self.project();
        if *proj.working {
            return EventStatePoll::Pending;
        }
        loop {
            match proj.receiver.as_mut().poll_next(cx) {
                Poll::Ready(Some((event_time, event))) => {
                    proj.events.insert(event_time, event);
                }
                Poll::Ready(None) => {
                    *proj.done = true;
                    break;
                }
                Poll::Pending => break,
            }
        }
        let (min, new_events) = proj.events.without_min_with_key();
        match min {
            Some((event_time, event)) => {
                if event_time <= poll_time {
                    *proj.events = new_events;
                    EventStatePoll::Event(event_time, event)
                } else {
                    EventStatePoll::Scheduled(event_time, proj.current_state.clone())
                }
            }
            None => match *proj.done {
                true => EventStatePoll::Done(proj.current_state.clone()),
                false => EventStatePoll::Ready(proj.current_state.clone()),
            },
        }
    }
}
