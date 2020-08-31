use super::{
    schedule_stream::{SchedulePoll, ScheduleStream},
    timestamp::Timestamp,
};
use crate::utilities::debug_waker::wrap_waker;
use futures::{Future, Stream};
use pin_project::pin_project;
use std::{
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll},
    time::Instant,
};
use tokio::time::{delay_for, Delay};

/// Stream for the [`to_realtime`](super::schedule_stream_ext::ScheduleStreamExt::to_realtime) method.
#[pin_project]
pub struct RealtimeStream<St: ScheduleStream>
where
    St::Time: Timestamp,
{
    reference: <St::Time as Timestamp>::Reference,
    #[pin]
    stream: St,
    #[pin]
    delay: Delay,
    current_waker_count: AtomicUsize,
}

impl<St: ScheduleStream> RealtimeStream<St>
where
    St::Time: Timestamp,
{
    pub(super) fn new(stream: St, reference: <St::Time as Timestamp>::Reference) -> Self {
        Self {
            stream,
            reference,
            delay: delay_for(std::time::Duration::from_secs(0)),
            current_waker_count: AtomicUsize::from(0),
        }
    }
}

impl<St: ScheduleStream> Stream for RealtimeStream<St>
where
    St::Time: Timestamp,
{
    type Item = St::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        println!("poll");
        let mut this = self.project();

        // START DEBUG STUFF
        let w = wrap_waker(
            cx.waker().to_owned(),
            this.current_waker_count.fetch_add(1, Ordering::SeqCst),
        );
        let cx = &mut Context::from_waker(&w);
        // END DEBUG STUFF

        let time = St::Time::get_timestamp(&Instant::now(), this.reference);

        let result = match this.stream.poll_next(time, cx) {
            SchedulePoll::Ready(p) => Poll::Ready(Some(p)),
            SchedulePoll::Scheduled(new_time) => {
                let instant = new_time.get_instant(&this.reference);
                let instant = tokio::time::Instant::from_std(instant);
                this.delay.reset(instant);

                Poll::Pending
            }
            SchedulePoll::Pending => Poll::Pending,
            SchedulePoll::Done => Poll::Ready(None),
        };

        // make sure the delay has a reference to the current waker.
        let _ = this.delay.poll(cx);

        result
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}
