use super::{EventStatePoll, EventStateStream};
use std::{iter::Peekable, pin::Pin, task::Context};

pub struct IterEventStateStream<I, T: Ord + Copy + Unpin, E: Unpin, S: Clone + Unpin>
where
    I: Iterator<Item = (T, E, S)> + Unpin,
{
    iter: Peekable<I>,
    previous_state: S,
}

impl<I, T: Ord + Copy + Unpin, E: Unpin, S: Clone + Unpin> IterEventStateStream<I, T, E, S>
where
    I: Iterator<Item = (T, E, S)> + Unpin,
{
    pub fn new(iter: I, initial_state: S) -> Self {
        Self {
            iter: iter.peekable(),
            previous_state: initial_state,
        }
    }
}

impl<I, T: Ord + Copy + Unpin, E: Unpin, S: Clone + Unpin> EventStateStream
    for IterEventStateStream<I, T, E, S>
where
    I: Iterator<Item = (T, E, S)> + Unpin,
{
    type Time = T;

    type Event = E;

    type State = S;

    fn poll(
        self: Pin<&mut Self>,
        poll_time: Self::Time,
        _cx: &mut Context<'_>,
    ) -> EventStatePoll<Self::Time, Self::Event, Self::State> {
        let this = self.get_mut();
        if let Some((t, ..)) = this.iter.peek() {
            let next_time = *t;
            if next_time <= poll_time {
                let (t, e, s) = this.iter.next().unwrap();
                this.previous_state = s;
                EventStatePoll::Event(t, e)
            } else {
                EventStatePoll::Scheduled(next_time, this.previous_state.clone())
            }
        } else {
            EventStatePoll::Ready(this.previous_state.clone())
        }
    }
}
