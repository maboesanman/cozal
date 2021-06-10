use either::Either;

use crate::source::adapters::{
    bounded, unbounded, LeftSplit, Map, RightSplit, Transposer, TransposerEngine,
};

use super::Source;
#[cfg(realtime)]
use super::{timestamp::Timestamp, RealtimeStream};

impl<S> SourceExt for S where S: Source {}

pub trait SourceExt: Source {
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
    fn into_engine<
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
        Self: Sized,
    {
        TransposerEngine::new(self, initial, rng_seed)
    }

    /// Adapter for converting the events and states of a stream.
    fn map<E, S>(
        self,
        event_transform: fn(Self::Event) -> E,
        state_transform: fn(Self::State) -> S,
    ) -> Map<Self, E, S>
    where
        Self: Sized,
    {
        Map::new(self, event_transform, state_transform)
    }
}

impl<S, L, R> EitherStateStreamExt<L, R> for S where S: Source<Event = Either<L, R>> {}

pub trait EitherStateStreamExt<L, R>: Source<Event = Either<L, R>> {
    /// Adapter to split a stream of Either<L, R> events into two streams.
    /// both streams have the full state, but each gets only L or R.
    ///
    /// this variant will return pending on one half if the other gets too far ahead.
    fn split_bounded(self, buffer_size: usize) -> (LeftSplit<Self, L, R>, RightSplit<Self, L, R>)
    where
        Self: Sized,
    {
        bounded(self, buffer_size)
    }

    /// Adapter to split a stream of Either<L, R> events into two streams.
    /// both streams have the full state, but each gets only L or R.
    fn split_unbounded(self) -> (LeftSplit<Self, L, R>, RightSplit<Self, L, R>)
    where
        Self: Sized,
    {
        unbounded(self)
    }
}
