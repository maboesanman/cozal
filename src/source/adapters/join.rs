use core::pin::Pin;
use core::task::Context;
use std::collections::HashMap;

use crate::source::{traits::StatelessSource, Source, SourcePoll};

pub struct Join<K, T: Ord + Copy, E, S> {
    sources: HashMap<K, JoinSource<T, E, S>>,
}

enum JoinSource<T, E, S> {
    Stateful(Box<dyn Source<Time = T, Event = E, State = S>>),
    Stateless(Box<dyn StatelessSource<Time = T, Event = E>>),
}

impl<K, T: Ord + Copy, E, S> Join<K, T, E, S> {
    pub fn new<Src>(_source: Src, _key: K) -> Self
    where
        Src: Source<Time = T, Event = E, State = S>,
    {
        unimplemented!()
    }

    pub fn new_stateless<Src>(_source: Src, _key: K) -> Self
    where
        Src: StatelessSource<Time = T, Event = E>,
    {
        unimplemented!()
    }

    pub fn join<Src>(&mut self, _new_source: Src, _new_key: K) -> Result<(), ()>
    where
        Src: Source<Time = T, Event = E, State = S>,
    {
        unimplemented!()
    }

    pub fn stateless_join<Src>(&mut self, _new_source: Src, _new_key: K) -> Result<(), ()>
    where
        Src: StatelessSource<Time = T, Event = E>,
    {
        unimplemented!()
    }
}

impl<K, T: Ord + Copy, E, S> Source for Join<K, T, E, S> {
    type Time = T;
    type Event = E;
    type State = HashMap<K, S>;

    fn poll(
        self: Pin<&mut Self>,
        _time: Self::Time,
        _cx: &mut Context<'_>,
    ) -> SourcePoll<Self::Time, Self::Event, Self::State> {
        unimplemented!()
    }
}
