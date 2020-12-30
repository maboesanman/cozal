use super::{StatefulSchedulePoll, StatefulScheduleStream};
use pin_project::pin_project;
use std::pin::Pin;
use std::task::Context;

#[pin_project]
pub struct Peekable<St: StatefulScheduleStream> {
    #[pin]
    stream: St,
    peeked: Option<StatefulSchedulePoll<St::Time, St::Item, St::State>>,
}

impl<St: StatefulScheduleStream> Peekable<St> {
    pub fn new(stream: St) -> Self {
        Self {
            stream,
            peeked: None,
        }
    }

    pub fn peek(
        self: Pin<&mut Self>,
        time: St::Time,
        cx: &mut Context<'_>,
    ) -> &StatefulSchedulePoll<St::Time, St::Item, St::State> {
        let this = self.project();
        if this.peeked.is_none() {
            *this.peeked = Some(this.stream.poll(time, cx));
        }
        this.peeked.as_ref().unwrap()
    }
}

impl<St: StatefulScheduleStream> StatefulScheduleStream for Peekable<St> {
    type Time = St::Time;

    type Item = St::Item;

    type State = St::State;

    fn poll(
        self: Pin<&mut Self>,
        poll_time: Self::Time,
        cx: &mut Context<'_>,
    ) -> StatefulSchedulePoll<Self::Time, Self::Item, Self::State> {
        let this = self.project();
        match std::mem::take(this.peeked) {
            None => this.stream.poll(poll_time, cx),
            Some(peeked) => match peeked {
                StatefulSchedulePoll::Ready(emitted_time, p, s) => {
                    if emitted_time > poll_time {
                        panic!()
                    }
                    StatefulSchedulePoll::Ready(emitted_time, p, s)
                }
                StatefulSchedulePoll::Scheduled(_scheduled_time, _s) => {
                    this.stream.poll(poll_time, cx)
                }
                StatefulSchedulePoll::Waiting(_peek_time) => this.stream.poll(poll_time, cx),
                StatefulSchedulePoll::Pending => StatefulSchedulePoll::Pending,
                StatefulSchedulePoll::Done(s) => StatefulSchedulePoll::Done(s),
            },
        }
    }
}
