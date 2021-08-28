use core::pin::Pin;
use core::task::Context;
use std::collections::HashMap;

use crate::source::{Source, SourcePoll, traits::{SourceContext, StatelessSource}};

pub struct Join<const CHANNELS: usize, K, T: Ord + Copy, E, S> {
    sources: HashMap<K, JoinSource<CHANNELS, T, E, S>>,
}

enum JoinSource<const CHANNELS: usize, T, E, S> {
    Stateful(Box<dyn Source<CHANNELS, Time = T, Event = E, State = S>>),
    Stateless(Box<dyn StatelessSource<CHANNELS, Time = T, Event = E>>),
}

impl<const CHANNELS: usize, K, T: Ord + Copy, E, S> Join<CHANNELS, K, T, E, S> {
    pub fn new<Src>(_source: Src, _key: K) -> Self
    where
        Src: Source<CHANNELS, Time = T, Event = E, State = S>,
    {
        unimplemented!()
    }

    pub fn new_stateless<Src>(_source: Src, _key: K) -> Self
    where
        Src: StatelessSource<CHANNELS, Time = T, Event = E>,
    {
        unimplemented!()
    }

    pub fn join<Src>(&mut self, _new_source: Src, _new_key: K) -> Result<(), ()>
    where
        Src: Source<CHANNELS, Time = T, Event = E, State = S>,
    {
        unimplemented!()
    }

    pub fn stateless_join<Src>(&mut self, _new_source: Src, _new_key: K) -> Result<(), ()>
    where
        Src: StatelessSource<CHANNELS, Time = T, Event = E>,
    {
        unimplemented!()
    }
}

impl<const CHANNELS: usize, K, T: Ord + Copy, E, S> Source<CHANNELS> for Join<CHANNELS, K, T, E, S> {
    type Time = T;
    type Event = E;
    type State = HashMap<K, S>;

    fn poll(
        self: Pin<&mut Self>,
        _time: Self::Time,
        _cx: &mut SourceContext<'_, CHANNELS, Self::Time>,
    ) -> SourcePoll<Self::Time, Self::Event, Self::State> {
        unimplemented!()
    }
}
