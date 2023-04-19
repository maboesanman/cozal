use std::pin::{pin, Pin};
use std::task::{Context, Poll};
use std::time::Instant;

use futures_core::{Future, Stream};

use crate::source_poll::Interrupt;
use crate::Source;

pub struct InterruptStream<Src: Source<Time = Instant>, Fut: Future<Output = ()>> {
    source:               Box<Src>,
    current_polling_time: Option<Instant>,
    current_wait:         Option<(Instant, Pin<Box<Fut>>)>,
    wait_fn:              fn(Instant) -> Fut,
}

impl<Src: Source<Time = Instant>, Fut: Future<Output = ()>> InterruptStream<Src, Fut> {
    pub fn new(source: Src, wait_fn: fn(Instant) -> Fut) -> Self {
        Self {
            source: Box::new(source),
            current_polling_time: None,
            current_wait: None,
            wait_fn,
        }
    }
}

impl<Src: Source<Time = Instant>, Fut: Future<Output = ()>> Stream for InterruptStream<Src, Fut> {
    type Item = (Instant, Interrupt<Src::Event>);

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // let poll_time = self.current_polling_time.unwrap_or_else(Instant::now);
        let poll_time = Instant::now();
        let this = self.get_mut();
        let poll = this.source.poll_events(poll_time, cx.waker().clone());

        let poll = match poll {
            Ok(poll) => poll,
            Err(_) => panic!(),
        };

        let next_event_at = match poll {
            crate::SourcePoll::Ready {
                state: _,
                next_event_at,
            } => next_event_at,
            crate::SourcePoll::Interrupt {
                time,
                interrupt,
            } => return Poll::Ready(Some((time, interrupt))),
            crate::SourcePoll::Pending => return Poll::Pending,
        };

        let next_event_at = match next_event_at {
            Some(next) => next,
            None => return Poll::Pending,
        };

        if this.current_wait.is_none() {
            this.current_wait = Some((next_event_at, Box::pin((this.wait_fn)(next_event_at))));
        }

        let (_, fut) = this.current_wait.as_mut().unwrap();

        match fut.as_mut().poll(cx) {
            Poll::Ready(()) => {
                this.current_wait = None;
                Pin::new(this).poll_next(cx)
            },
            Poll::Pending => Poll::Pending,
        }
    }
}
