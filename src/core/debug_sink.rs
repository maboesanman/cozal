use core::task::{Context, Poll};
use core::pin::Pin;
use futures::Sink;
use core::fmt::Debug;

use super::event::{Event};

pub struct DebugSink { }

impl<In: Clone + Debug> Sink<Event<In>> for DebugSink {
    type Error = ();

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: Event<In>) -> Result<(), Self::Error> {
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