

use pin_project::pin_project;
use futures::Future;
use std::{task::{Poll, Context}, pin::Pin};
use super::debug_waker::DebugWakerFactory;

#[pin_project]
pub struct DebugFuture<F: Future> {
    name: String,
    #[pin]
    fut: F,
    waker_factory: DebugWakerFactory,
}

impl<F: Future>  DebugFuture<F> {
    pub fn new(fut: F, name: &str) -> Self {
        DebugFuture {
            name: name.to_string(),
            fut,
            waker_factory: DebugWakerFactory::new(name)
        }
    }
}

impl<F: Future> Future for DebugFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        println!("poll {}", this.name);
        let w = this.waker_factory.wrap_waker(cx.waker().to_owned());
        let cx = &mut Context::from_waker(&w);

        this.fut.poll(cx)
    }
}