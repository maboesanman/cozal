use std::pin::{pin, Pin};
use std::task::{Context, Poll};
use std::time::Instant;

use crate::source_poll::Interrupt;
use crate::Source;

pub struct InterruptIterator<Src: Source<Time = Instant>> {
    source:               Box<Src>,
    current_polling_time: Option<Instant>,
}

impl<Src: Source<Time = Instant>> InterruptIterator<Src> {
    pub fn new(source: Src) -> Self {
        Self {
            source:               Box::new(source),
            current_polling_time: None,
        }
    }
}

impl<Src: Source<Time = Instant>> Iterator for InterruptIterator<Src> {
    type Item = (Instant, Interrupt<Src::Event>);

    fn next(&mut self) -> Option<Self::Item> {
        let poll_time = self.current_polling_time.unwrap_or_else(Instant::now);
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

        let wait_fut = (this.wait_fn)(next_event_at);

        match pin!(wait_fut).poll(cx) {
            Poll::Ready(()) => Pin::new(this).poll_next(cx),
            Poll::Pending => Poll::Pending,
        }
    }
}
