use core::fmt::Debug;
use core::pin::Pin;
use core::task::{Context, Poll};
use futures::Sink;

// todo document.
pub struct DebugSink {}

impl DebugSink {
    pub fn new() -> Self {
        DebugSink {}
    }
}

impl<T: Clone + Debug> Sink<T> for DebugSink {
    type Error = ();

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        println!("{:?}", item);
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}
