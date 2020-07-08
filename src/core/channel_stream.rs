use std::sync::{Arc, Mutex};
use core::task::{Context, Poll, Waker};
use core::pin::Pin;
use futures::Stream;
use std::sync::mpsc::{sync_channel, SyncSender, Receiver, TryRecvError};

pub fn channel_stream<T>() -> (SenderSink<T>, ReceiverStream<T>) {
    let (s, r) = sync_channel(2048);
    let w1 = Arc::new(Mutex::new(None));
    let w2 = w1.clone();
    (SenderSink::new(s, w1), ReceiverStream::new(r, w2))
}

pub struct SenderSink<T> {
    waker: Arc<Mutex<Option<Waker>>>,
    sender: SyncSender<Option<T>>
}

impl<T> SenderSink<T> {
    pub fn new(sender: SyncSender<Option<T>>, waker: Arc<Mutex<Option<Waker>>>) -> Self {
        SenderSink {
            waker,
            sender
        }
    }

    pub fn enque(&self, item: T) -> Result<(), ()> {
        self.send(Some(item))
    }

    pub fn close(&self) -> Result<(), ()> {
        self.send(None)
    }

    fn send(&self, item: Option<T>) -> Result<(), ()> {
        match self.sender.send(item) {
            Ok(_) => {
                self.wake();
                Ok(())
            },
            Err(_) => Err(())
        }
    }

    fn wake(&self) {
        match self.waker.lock() {
            Ok(waker) => {
                match &*waker {
                    Some(w) => w.wake_by_ref(),
                    None => {}
                }
            },
            Err(_) => {},
        }
    }
}

pub struct ReceiverStream<T> {
    waker: Arc<Mutex<Option<Waker>>>,
    receiver: Receiver<Option<T>>,
}

impl<T> ReceiverStream<T> {
    pub fn new(receiver: Receiver<Option<T>>, waker: Arc<Mutex<Option<Waker>>>) -> Self {
        ReceiverStream {
            waker,
            receiver
        }
    }
}

impl<T: Sync + Unpin> Stream for ReceiverStream<T> {
    type Item = Result<T, ()>;
    
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Result<T, ()>>> {
        match self.receiver.try_recv() {
            Ok(Some(item)) => Poll::Ready(Some(Ok(item))),
            Ok(None) => Poll::Ready(None),
            Err(TryRecvError::Disconnected) => Poll::Ready(None),
            Err(TryRecvError::Empty) => {
                match self.waker.lock() {
                    Ok(mut waker) => {
                        *waker = Some(cx.waker().clone());
                        Poll::Pending
                    },
                    Err(_) => Poll::Ready(None),
                }
            },
        }
    }
}