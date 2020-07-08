use core::task::{Context, Poll};
use core::pin::Pin;
use futures::Sink;
use core::fmt::Debug;

pub struct DebugSink { }

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
        // here is where rollbacks happen
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