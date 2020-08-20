use core::pin::Pin;
use futures::task::{Context, Poll, Waker};
use crate::core::event::event::{EventTimestamp, Event};
use super::transposer::Transposer;
use futures::{Future, Stream};
use std::sync::{Mutex, Arc};
use core::sync::atomic::Ordering::Relaxed;
use core::sync::atomic::AtomicUsize;
use super::transposer_engine_internal::TransposerEngineInternal;



pub struct TransposerEngine<'a, T: Transposer + 'a, S: Stream<Item = Event<T::External>> + Unpin + 'a> {
    internal: Arc<Mutex<TransposerEngineInternal<'a, T, S>>>,
    current_poll_stream: Arc<AtomicUsize>,
}

#[allow(dead_code)]
impl<'a, T: Transposer + 'a, S: Stream<Item = Event<T::External>> + Unpin + 'a> TransposerEngine<'a, T, S> {
    pub async fn new(input_stream: S) -> TransposerEngine<'a, T, S> {
        let internal = TransposerEngineInternal::new(input_stream).await;
        let internal = Arc::new(Mutex::new(internal));

        let current_poll_stream = Arc::new(AtomicUsize::from(0));
        TransposerEngine {
            internal,
            current_poll_stream,
        }
    }

    pub fn poll(&self, t: EventTimestamp) -> TransposerEngineStream<'a, T, S> {
        // Let the current pending stream know to wake up. It will resolve to None.
        match self.internal.lock() {
            Ok(internal) => {
                if let Some(w) = &internal.current_waker {
                    w.wake_by_ref();
                }
            }
            Err(_) => panic!()
        };
        TransposerEngineStream {
            internal: self.internal.clone(),
            poll_stream_id: self.current_poll_stream.fetch_add(1, Relaxed),
            current_poll_stream: self.current_poll_stream.clone(),
            until: t,
        }
    }
}

pub struct TransposerEngineStream<'a, T: Transposer + 'a, S: Stream<Item = Event<T::External>> + Unpin + 'a> {
    internal: Arc<Mutex<TransposerEngineInternal<'a, T, S>>>,
    poll_stream_id: usize,
    current_poll_stream: Arc<AtomicUsize>,
    until: EventTimestamp,
}

impl<'a, T: Transposer + 'a, S: Stream<Item = Event<T::External>> + Unpin + 'a> Stream for TransposerEngineStream<'a, T, S> {
    type Item = Event<T::Out>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.current_poll_stream.load(Relaxed) != self.poll_stream_id {
            return Poll::Ready(None);
        }

        // someday do this locking with futures...
        match self.internal.lock() {
            Ok(mut internal) => {
                internal.poll(cx, &self.until)
            }
            Err(_) => panic!()
        }
    }
}
