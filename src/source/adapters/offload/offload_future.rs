use std::{pin::Pin, task::{Context, Poll}};

use futures_core::Future;

use crate::source::Source;

pub struct OffloadFuture<Src: Source> {
    source: Src,
}

impl<Src: Source> Future for OffloadFuture<Src> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unimplemented!()
    }
}
