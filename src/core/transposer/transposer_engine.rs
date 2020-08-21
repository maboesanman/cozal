use super::transposer::Transposer;
use super::transposer_engine_internal::TransposerEngineInternal;
use crate::core::event::event::Event;
use core::pin::Pin;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::SeqCst;
use futures::task::{Context, Poll};
use futures::Stream;
use std::sync::{Arc, Mutex};

pub struct TransposerEngine<
    'a,
    T: Transposer + 'a,
    S: Stream<Item = Event<T::Time, T::External>> + Unpin + Send + 'a,
> {
    internal: Arc<Mutex<TransposerEngineInternal<'a, T, S>>>,
    current_poll_stream: Arc<AtomicUsize>,
}

#[allow(dead_code)]
impl<'a, T: Transposer + 'a, S: Stream<Item = Event<T::Time, T::External>> + Unpin + Send + 'a>
    TransposerEngine<'a, T, S>
{
    pub async fn new(input_stream: S) -> TransposerEngine<'a, T, S> {
        let internal = TransposerEngineInternal::new(input_stream).await;
        let internal = Arc::new(Mutex::new(internal));

        let current_poll_stream = Arc::new(AtomicUsize::from(0));
        TransposerEngine {
            internal,
            current_poll_stream,
        }
    }

    pub fn poll(&self, t: T::Time) -> TransposerEngineStream<'a, T, S> {
        // Let the current pending stream know to wake up. It will resolve to None.
        match self.internal.lock() {
            Ok(internal) => {
                if let Some(w) = &internal.current_waker {
                    w.wake_by_ref();
                }
            }
            Err(_) => panic!(),
        };
        TransposerEngineStream {
            internal: self.internal.clone(),
            poll_stream_id: self.current_poll_stream.fetch_add(1, SeqCst) + 1,
            current_poll_stream: self.current_poll_stream.clone(),
            until: t,
        }
    }
}

pub struct TransposerEngineStream<
    'a,
    T: Transposer + 'a,
    S: Stream<Item = Event<T::Time, T::External>> + Unpin + Send + 'a,
> {
    internal: Arc<Mutex<TransposerEngineInternal<'a, T, S>>>,
    poll_stream_id: usize,
    current_poll_stream: Arc<AtomicUsize>,
    until: T::Time,
}

impl<'a, T: Transposer + 'a, S: Stream<Item = Event<T::Time, T::External>> + Unpin + Send> Stream
    for TransposerEngineStream<'a, T, S>
{
    type Item = Event<T::Time, T::Out>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.current_poll_stream.load(SeqCst) != self.poll_stream_id {
            return Poll::Ready(None);
        }

        // someday do this locking with futures...
        match self.internal.lock() {
            Ok(mut internal) => internal.poll(cx, &self.until),
            Err(_) => panic!(),
        }
    }
}
