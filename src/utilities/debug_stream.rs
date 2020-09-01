

use pin_project::pin_project;
use futures::{Stream};
use std::{task::{Poll, Context}, pin::Pin};
use super::debug_waker::DebugWakerFactory;

#[pin_project]
pub struct DebugStream<S: Stream> {
    name: String,
    #[pin]
    stream: S,
    waker_factory: DebugWakerFactory,
}

impl<S: Stream> DebugStream<S> {
    pub fn new(stream: S, name: &str) -> Self {
        DebugStream {
            name: name.to_string(),
            stream,
            waker_factory: DebugWakerFactory::new(name)
        }
    }
}

impl<S: Stream> Stream for DebugStream<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        println!("poll_next {}", this.name);
        let w = this.waker_factory.wrap_waker(cx.waker().to_owned());
        let cx = &mut Context::from_waker(&w);

        this.stream.poll_next(cx)
    }
}