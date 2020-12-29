use super::{StatefulSchedulePoll, StatefulScheduleStream};
use futures::Stream;
use pin_project::pin_project;
use std::{
    pin::Pin,
    task::{Context, Poll},
};

/// Stream for the [`to_realtime`](super::schedule_stream_ext::ScheduleStreamExt::to_realtime) method.
#[pin_project]
pub struct TargetStream<St: StatefulScheduleStream> {
    #[pin]
    stream: St,
    target: St::Time,
    next_target: Option<St::Time>,
    state: Option<St::State>,
}

impl<St: StatefulScheduleStream> TargetStream<St> {
    pub(super) fn new(stream: St, target: St::Time) -> Self {
        Self {
            stream,
            target,
            next_target: None,
            state: None,
        }
    }

    pub fn set_target(self: Pin<&mut Self>, target: St::Time) {
        *self.project().target = target;
    }

    pub fn get_next_target(self: Pin<&mut Self>) -> Option<St::Time> {
        self.into_ref().get_ref().next_target
    }

    pub fn into_inner(self) -> St {
        self.stream
    }
}

impl<St: StatefulScheduleStream> Stream for TargetStream<St> {
    type Item = (St::Time, St::Item, St::State);

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        *this.next_target = None;

        match this.stream.poll(*this.target, cx) {
            StatefulSchedulePoll::Ready(t, p, s) => Poll::Ready(Some((t, p, s))),
            StatefulSchedulePoll::Scheduled(t, _) => {
                *this.next_target = Some(t);

                Poll::Pending
            }
            StatefulSchedulePoll::Waiting(_) => Poll::Pending,
            StatefulSchedulePoll::Pending => Poll::Pending,
            StatefulSchedulePoll::Done(_) => Poll::Ready(None),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}
