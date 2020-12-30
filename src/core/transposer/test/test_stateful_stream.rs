use flume::{unbounded, Receiver, Sender};
use futures::Stream;
use im::OrdMap;
use pin_project::pin_project;
use std::{pin::Pin, task::Poll};

use crate::core::schedule_stream::{StatefulSchedulePoll, StatefulScheduleStream};

#[pin_project]
pub struct TestStatefulStream<Time: Ord + Copy, Item: Clone, State: Clone> {
    #[pin]
    receiver: Receiver<(Time, Item)>,

    events: OrdMap<Time, Item>,
    current_state: State,
    done: bool,
    working: bool,
}

impl<Time: Ord + Copy, Item: Clone, State: Clone> TestStatefulStream<Time, Item, State> {
    pub fn new(current_state: State) -> (Sender<(Time, Item)>, Self) {
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

impl<Time: Ord + Copy, Item: Clone, State: Clone> StatefulScheduleStream
    for TestStatefulStream<Time, Item, State>
{
    type Time = Time;

    type Item = Item;

    type State = State;

    fn poll(
        self: Pin<&mut Self>,
        poll_time: Self::Time,
        cx: &mut std::task::Context<'_>,
    ) -> crate::core::schedule_stream::StatefulSchedulePoll<Self::Time, Self::Item, Self::State>
    {
        let mut proj = self.project();
        if *proj.working {
            return StatefulSchedulePoll::Pending;
        }
        loop {
            match proj.receiver.as_mut().poll_next(cx) {
                Poll::Ready(Some((event_time, item))) => {
                    proj.events.insert(event_time, item);
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
            Some((event_time, item)) => {
                if event_time <= poll_time {
                    *proj.events = new_events;
                    StatefulSchedulePoll::Ready(event_time, item, proj.current_state.clone())
                } else {
                    StatefulSchedulePoll::Scheduled(event_time, proj.current_state.clone())
                }
            }
            None => match *proj.done {
                true => StatefulSchedulePoll::Done(proj.current_state.clone()),
                false => StatefulSchedulePoll::Waiting(proj.current_state.clone()),
            },
        }
    }
}
