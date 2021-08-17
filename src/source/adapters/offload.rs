use std::{pin::Pin, task::{Context, Poll}};

use futures_core::Future;

use crate::source::Source;

struct OffloadInner<Src: Source> {
    source: Src
}

pub struct OffloadSource<Src: Source> {
    inner: OffloadInner<Src>
}

pub struct OffloadWork<Src: Source> {
    inner: OffloadInner<Src>
}

pub fn offload<Src: Source>(source: Src) -> (OffloadSource<Src>, OffloadWork<Src>) {
    unimplemented!()
}

impl<Src: Source> Source for OffloadSource<Src> {
    type Time = Src::Time;

    type Event = Src::Event;

    type State = Src::State;

    fn poll(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: &mut Context<'_>,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, Self::State> {
        unimplemented!()
    }
}

impl<Src: Source> Future for OffloadWork<Src> {
    type Output = !;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unimplemented!()
    }
}