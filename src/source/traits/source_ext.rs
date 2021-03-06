use std::time::Instant;

use futures_core::Future;

use crate::source::adapters::{
    realtime, Duplicate, Join, Map, RealtimeEvents, RealtimeStates, Shift, Split, Transposer,
    TransposerEngine,
};

use super::{Source, Timestamp};

impl<S> SourceExt for S where S: Source {}

pub trait SourceExt: Source + Sized {
    /// Adapter for converting a schedule_stream into one that yields the items in realtime, and a struct which can be queried yielding
    ///
    /// a reference must be given for the conversion from [`S::Time`](super::schedule_stream::ScheduleStream::Time) to [`Instant`](std::time::Instant).
    /// for example if you use [`Duration`](core::time::Duration) as your time, a "start time" must be given
    /// so that the duration can be added to something.
    ///
    /// if your timestamp is an [`Instant`](std::time::Instant), then your reference is of type `()` because instants
    /// are already realtime and need no reference.
    ///
    /// Additionally, a sleep_fn must be provided. This should be simply `tokio::time::sleep_until` if using tokio, but is left generic to avoid requiring a runtime.
    fn realtime<SleepFut: Future, SleepFn: Fn(Instant) -> SleepFut>(
        self,
        reference: <Self::Time as Timestamp>::Reference,
        sleep_fn: SleepFn,
    ) -> (RealtimeEvents<Self>, RealtimeStates<Self>)
    where
        Self::Time: Timestamp,
    {
        realtime(self, reference, sleep_fn)
    }

    /// Adapter for converting a schedule stream into another via a transposer.
    fn transpose<
        'tr,
        T: Transposer<Time = Self::Time, Input = Self::Event, InputState = Self::State> + 'tr,
        const N: usize,
    >(
        self,
        initial: T,
        rng_seed: [u8; 32],
    ) -> TransposerEngine<'tr, T, Self, N>
    where
        T: Clone,
        T::Scheduled: Clone,
    {
        TransposerEngine::new(self, initial, rng_seed)
    }

    /// Adapter for converting the events and states of a stream.
    fn map<E, S>(
        self,
        event_transform: fn(Self::Event) -> E,
        state_transform: fn(Self::State) -> S,
    ) -> Map<Self, E, S> {
        Map::new(self, event_transform, state_transform)
    }

    fn shift<T: Ord + Copy, IntoNew, IntoOld>(
        self,
        into_new: fn(Self::Time) -> T,
        into_old: fn(T) -> Self::Time,
    ) -> Shift<Self, T> {
        Shift::new(self, into_new, into_old)
    }

    fn joinable<K>(self, self_key: K) -> Join<K, Self::Time, Self::Event, Self::State> {
        Join::new(self, self_key)
    }

    fn stateless_joinable<K>(self, self_key: K) -> Join<K, Self::Time, Self::Event, Self::State> {
        Join::new_stateless(self, self_key)
    }

    fn splittable<E, ConvertFn>(
        self,
        decide: fn(&Self::Event) -> bool,
        convert: ConvertFn,
    ) -> Split<Self, E, ConvertFn>
    where
        ConvertFn: Fn(Self::Event) -> E,
    {
        Split::new(self, decide, convert)
    }

    fn duplicate(self) -> Duplicate<Self>
    where
        Self::Event: Clone,
    {
        Duplicate::new(self)
    }
}
