use std::{marker::PhantomData, pin::Pin};

use crate::source::{Source, traits::SourceContext};



pub struct OffloadSource<Src: Source> {
    phantom: PhantomData<Src>
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

    fn advance(self: Pin<&mut Self>, time: Self::Time) {
        todo!()
    }

    fn max_channel(&self) -> std::num::NonZeroUsize {
        todo!()
    }
}
