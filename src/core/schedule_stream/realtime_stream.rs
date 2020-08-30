use super::{
    schedule_stream::{SchedulePoll, ScheduleStream},
    timestamp::Timestamp,
};
use futures::{Future, Stream};
use pin_project::pin_project;
use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Instant,
};
use tokio::time::{Delay, delay_for};

/// Stream for the `to_realtime` method.
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
        }
    }
}

impl<St: ScheduleStream> Stream for RealtimeStream<St>
where
    St::Time: Timestamp,
{
    type Item = St::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
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
}
