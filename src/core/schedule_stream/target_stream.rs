use super::{SchedulePoll, StatefulScheduleStream};
use futures::Stream;
use pin_project::pin_project;
use std::{
    pin::Pin,
    sync::Mutex,
    task::{Context, Poll},
};

/// Stream for the [`to_realtime`](super::schedule_stream_ext::ScheduleStreamExt::to_realtime) method.
#[pin_project]
pub struct TargetStream<St: StatefulScheduleStream> {
    #[pin]
    stream: St,
    target: Mutex<St::Time>,
    next_target: Option<St::Time>,
}

impl<St: StatefulScheduleStream> TargetStream<St> {
    pub(super) fn new(stream: St, target: St::Time) -> Self {
        Self {
            stream,
            target: Mutex::new(target),
            next_target: None,
        }
    }

    pub fn set_target(&self, target: St::Time) {
        *self.target.lock().unwrap() = target;
    }

    pub fn get_next_target(&self) -> Option<St::Time> {
        self.next_target
    }

    pub fn into_inner(self) -> St {
        self.stream
    }
}

impl<St: StatefulScheduleStream> Stream for TargetStream<St> {
    type Item = (St::Time, St::State, St::Item);

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        *this.next_target = None;

        match this.stream.poll(*this.target.lock().unwrap(), cx) {
            (s, SchedulePoll::Ready(t, p)) => Poll::Ready(Some((t, s, p))),
            (_, SchedulePoll::Scheduled(new_time)) => {
                *this.next_target = Some(new_time);

                Poll::Pending
            }
            (_, SchedulePoll::Pending) => Poll::Pending,
            (_, SchedulePoll::Done) => Poll::Ready(None),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}
