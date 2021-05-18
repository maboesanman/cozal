use super::{timestamp::Timestamp, EventStatePoll, EventStateStream};
use futures::{Future, Stream};
use pin_project::pin_project;
use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Instant,
};
use tokio::time::{delay_for, Delay};

/// Stream for the [`to_realtime`](super::schedule_stream_ext::ScheduleStreamExt::to_realtime) method.
#[pin_project]
pub struct Realtime<St: EventStateStream>
where
    St::Time: Timestamp,
{
    reference: <St::Time as Timestamp>::Reference,
    #[pin]
    stream: St,
    #[pin]
    delay: Delay,

    state: Option<St::State>,
}

impl<St: EventStateStream> Realtime<St>
where
    St::Time: Timestamp,
{
    pub(super) fn new(stream: St, reference: <St::Time as Timestamp>::Reference) -> Self {
        Self {
            stream,
            reference,
            delay: delay_for(std::time::Duration::from_secs(0)),
            state: None,
        }
    }
}

impl<St: EventStateStream> Stream for Realtime<St>
where
    St::Time: Timestamp,
{
    type Item = (St::Time, St::Item, St::State);

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        let time = St::Time::get_timestamp(&Instant::now(), this.reference);

        match this.stream.poll(time, cx) {
            EventStatePoll::Event(t, p, s) => {
                *this.state = None;
                Poll::Ready(Some((t, p, s)))
            }
            EventStatePoll::Scheduled(new_time, s) => {
                *this.state = Some(s);
                let instant = new_time.get_instant(&this.reference);
                let instant = tokio::time::Instant::from_std(instant);

                this.delay.reset(instant);
                let _ = this.delay.poll(cx);

                Poll::Pending
            }
            EventStatePoll::Ready(s) => {
                *this.state = Some(s);
                Poll::Pending
            }
            EventStatePoll::Pending => Poll::Pending,
            EventStatePoll::Done(s) => {
                *this.state = Some(s);
                Poll::Ready(None)
            }
            EventStatePoll::Rollback(_t) => todo!(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}
