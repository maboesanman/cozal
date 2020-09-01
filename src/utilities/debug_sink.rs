use core::fmt::Debug;
use core::pin::Pin;
use core::task::{Context, Poll};
use futures::Sink;

use pin_project::pin_project;
use super::debug_waker::DebugWakerFactory;
use std::marker::PhantomData;

#[pin_project]
pub struct DebugSink<T, S: Sink<T>> {
    name: String,
    #[pin]
    sink: S,
    waker_factory: DebugWakerFactory,

    phantom: std::marker::PhantomData<T>
}

impl<T, S: Sink<T>> DebugSink<T, S> {
    pub fn new(sink: S, name: &str) -> Self {
        DebugSink {
            name: name.to_string(),
            sink,
            waker_factory: DebugWakerFactory::new(name),
            phantom: PhantomData
        }
    }
}

impl<T, S: Sink<T>> Sink<T> for DebugSink<T, S> {
    type Error = S::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        println!("poll_ready {}", this.name);
        let w = this.waker_factory.wrap_waker(cx.waker().to_owned());
        let cx = &mut Context::from_waker(&w);

        this.sink.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        let this = self.project();
        println!("start_send {}", this.name);

        this.sink.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        println!("poll_flush {}", this.name);
        let w = this.waker_factory.wrap_waker(cx.waker().to_owned());
        let cx = &mut Context::from_waker(&w);

        this.sink.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        println!("poll_close {}", this.name);
        let w = this.waker_factory.wrap_waker(cx.waker().to_owned());
        let cx = &mut Context::from_waker(&w);

        this.sink.poll_close(cx)
    }
}
// todo document.
pub struct PrintSink {}

impl PrintSink {
    pub fn new() -> Self {
        PrintSink {}
    }
}

impl<T: Debug> Sink<T> for PrintSink {
    type Error = core::convert::Infallible;

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
