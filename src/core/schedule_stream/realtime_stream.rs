

use pin_project::{pin_project, project};
use super::{timestamp::Timestamp, schedule_stream::{SchedulePoll, ScheduleStream}};
use std::{pin::Pin, time::{Duration, Instant}, task::{Context, Poll}};
use futures::{Stream, Future};
use tokio::time::delay_until;
#[pin_project]
pub struct RealtimeStream<St: ScheduleStream>
where St::Time: Timestamp {
    pub(super) reference: <St::Time as Timestamp>::Reference,
    #[pin]
    pub(super) stream: St,
}

impl<St: ScheduleStream> Stream for RealtimeStream<St>
where St::Time: Timestamp {
    type Item = St::Item;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let time = St::Time::get_timestamp(&Instant::now(), this.reference);
        match this.stream.poll_next(time, cx) {
            SchedulePoll::Ready(p) => Poll::Ready(Some(p)),
            SchedulePoll::Scheduled(t) => {
                let time = St::Time::get_instant(&t, this.reference);

                // we create and poll a tokio delay so that the waker will be called at that time.
                let mut delay = delay_until(tokio::time::Instant::from_std(time));
                let _ = Pin::new(&mut delay).poll(cx);
                Poll::Pending
            }
            SchedulePoll::Pending => Poll::Pending,
            SchedulePoll::Done => Poll::Ready(None),
        }
    }
}