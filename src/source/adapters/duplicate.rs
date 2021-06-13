use std::sync::{Arc, Mutex};

use crate::source::Source;

pub struct Duplicate<Src: Source>
where
    Src::Event: Clone,
{
    source: Arc<Mutex<Src>>,
}

impl<Src: Source> Duplicate<Src>
where
    Src::Event: Clone,
{
    pub fn new(_source: Src) -> Self {
        unimplemented!()
    }
}

impl<Src: Source> Clone for Duplicate<Src>
where
    Src::Event: Clone,
{
    fn clone(&self) -> Self {
        unimplemented!()
    }
}

impl<Src: Source> Source for Duplicate<Src>
where
    Src::Event: Clone,
{
    type Time = Src::Time;

    type Event = Src::Event;

    type State = Src::State;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        _time: Self::Time,
        _cx: &mut std::task::Context<'_>,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, Self::State> {
        unimplemented!()
    }
}
