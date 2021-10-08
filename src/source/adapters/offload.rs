use core::pin::Pin;
use core::task::{Context, Poll};

use futures_core::Future;

use crate::source::traits::SourceContext;
use crate::source::Source;

struct OffloadInner<Src: Source> {
    source: Src,
}

pub struct OffloadSource<Src: Source> {
    inner: OffloadInner<Src>,
}

pub struct OffloadFuture<Src: Source> {
    inner: OffloadInner<Src>,
}

pub fn offload<Src: Source>(source: Src) -> (OffloadSource<Src>, OffloadFuture<Src>) {
    unimplemented!()
}

impl<Src: Source> Source for OffloadSource<Src> {
    type Time = Src::Time;

    type Event = Src::Event;

    type State = Src::State;

    type Error = Src::Error;

    fn poll(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: SourceContext,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, Self::State, Src::Error> {
        unimplemented!()
    }
}

impl<Src: Source> Future for OffloadFuture<Src> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unimplemented!()
    }
}
