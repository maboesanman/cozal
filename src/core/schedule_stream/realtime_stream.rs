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
use tokio::time::{delay_until, Delay};
#[pin_project]
pub struct RealtimeStream<St: ScheduleStream>
where
    St::Time: Timestamp,
{
    reference: <St::Time as Timestamp>::Reference,
    #[pin]
    stream: St,
    delay: Option<(St::Time, Delay)>,
}

impl<St: ScheduleStream> RealtimeStream<St>
where
    St::Time: Timestamp,
{
    pub fn new(stream: St, reference: <St::Time as Timestamp>::Reference) -> Self {
        Self {
            stream,
            reference,
            delay: None,
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
        let mut time = St::Time::get_timestamp(&Instant::now(), this.reference);

        if let Some((delay_time, delay)) = this.delay {
            if delay.is_elapsed() && time < *delay_time {
                time = *delay_time;
            }
        }

        match this.stream.poll_next(time, cx) {
            SchedulePoll::Ready(p) => Poll::Ready(Some(p)),
            SchedulePoll::Scheduled(time) => {
                let instant = time.get_instant(&this.reference);
                let instant = tokio::time::Instant::from_std(instant);
                *this.delay = Some((time, delay_until(instant)));

                if let Some(delay) = this.delay {
                    Pin::new(&mut delay.1).poll(cx);
                }
                Poll::Pending
            }
            SchedulePoll::Pending => Poll::Pending,
            SchedulePoll::Done => Poll::Ready(None),
        }
    }
}
