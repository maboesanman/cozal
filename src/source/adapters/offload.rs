use std::{pin::Pin, task::{Context, Poll}};

use futures_core::Future;

use crate::source::{Source, traits::SourceContext};

struct OffloadInner<const CHANNELS: usize, Src: Source<CHANNELS>> {
    source: Src
}

pub struct OffloadSource<const CHANNELS: usize, Src: Source<CHANNELS>> {
    inner: OffloadInner<CHANNELS, Src>
}

pub struct OffloadWork<const CHANNELS: usize, Src: Source<CHANNELS>> {
    inner: OffloadInner<CHANNELS, Src>
}

pub fn offload<
    const CHANNELS: usize,
    Src: Source<CHANNELS>,
>(source: Src) -> (
    OffloadSource<CHANNELS, Src>,
    OffloadWork<CHANNELS, Src>,
) {
    unimplemented!()
}

impl<
    const CHANNELS: usize,
    Src: Source<CHANNELS>,
> Source<CHANNELS> for OffloadSource<CHANNELS, Src> {
    type Time = Src::Time;

    type Event = Src::Event;

    type State = Src::State;

    fn poll(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: &mut SourceContext<'_, CHANNELS, Self::Time>,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, Self::State> {
        unimplemented!()
    }
}

impl<
    const CHANNELS: usize,
    Src: Source<CHANNELS>,
> Future for OffloadWork<CHANNELS, Src> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unimplemented!()
    }
}