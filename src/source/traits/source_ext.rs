use either::Either;

use crate::source::adapters::{Join, Map, Split, Transposer, TransposerEngine};

use super::Source;
#[cfg(realtime)]
use super::{timestamp::Timestamp, RealtimeStream};

impl<S> SourceExt for S where S: Source {}

pub trait SourceExt: Source + Sized {
    /// Adapter for converting a schedule_stream into one that yields the items in realtime.
    ///
    /// a reference must be given for the conversion from [`S::Time`](super::schedule_stream::ScheduleStream::Time) to [`Instant`](std::time::Instant).
    /// for example if you use [`Duration`](std::time::Duration) as your time, a "start time" must be given
    /// so that the duration can be added to something.
    ///
    /// if your timestamp is an [`Instant`](std::time::Instant), then your reference is of type `()` because instants
    /// are already realtime and need no reference.
    #[cfg(realtime)]
    fn into_realtime(self, reference: <Self::Time as Timestamp>::Reference) -> RealtimeStream<Self>
    where
        <Self as Source>::Time: Timestamp,
    {
        RealtimeStream::new(self, reference)
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
    ) -> Map<Self, E, S>
    {
        Map::new(self, event_transform, state_transform)
    }

    fn joinable<K>(self, self_key: K) -> Join<K, Self::Time, Self::Event, Self::State>
    {
        Join::new(self, self_key)
    }

    fn stateless_joinable<K>(self, self_key: K) -> Join<K, Self::Time, Self::Event, Self::State>
    {
        Join::new_stateless(self, self_key)
    }

    fn splittable<E, ConvertFn>(
        self,
        decide: fn(&Self::Event) -> bool,
        convert: ConvertFn
    ) -> Split<Self, E, ConvertFn>
    where
        ConvertFn: Fn(Self::Event) -> E
    {
        Split::new(self, decide, convert)
    }
}
