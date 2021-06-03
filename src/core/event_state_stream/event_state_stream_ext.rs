use either::Either;

use crate::core::{transposer::TransposerEngine, Transposer};

use super::{
    event_state_map_stream::EventStateMapStream,
    event_state_split_stream::{
        bounded, unbounded, EventStateSplitLeft, EventStateSplitRight,
    },
    EventStateStream,
};
#[cfg(realtime)]
use super::{timestamp::Timestamp, RealtimeStream};

impl<S> EventStateStreamExt for S where S: EventStateStream {}

pub trait EventStateStreamExt: EventStateStream {
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
        <Self as EventStateStream>::Time: Timestamp,
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
    ) -> TransposerEngine<'tr, T, Self, N>
    where
        T: Clone,
        T::Scheduled: Clone,
        Self: Sized,
    {
        TransposerEngine::new(self, initial)
    }

    /// Adapter for converting the events and states of a stream.
    fn map<E, S>(
        self,
        event_transform: fn(Self::Event) -> E,
        state_transform: fn(Self::State) -> S,
    ) -> EventStateMapStream<Self, E, S>
    where
        Self: Sized,
    {
        EventStateMapStream::new(self, event_transform, state_transform)
    }
}

impl<S, L, R> EitherStateStreamExt<L, R> for S where S: EventStateStream<Event = Either<L, R>> {}

pub trait EitherStateStreamExt<L, R>: EventStateStream<Event = Either<L, R>> {
    /// Adapter to split a stream of Either<L, R> events into two streams.
    /// both streams have the full state, but each gets only L or R.
    ///
    /// this variant will return pending on one half if the other gets too far ahead.
    fn split_bounded(
        self,
        buffer_size: usize,
    ) -> (
        EventStateSplitLeft<Self, L, R>,
        EventStateSplitRight<Self, L, R>,
    )
    where
        Self: Sized,
    {
        bounded(self, buffer_size)
    }

    /// Adapter to split a stream of Either<L, R> events into two streams.
    /// both streams have the full state, but each gets only L or R.
    fn split_unbounded(
        self,
    ) -> (
        EventStateSplitLeft<Self, L, R>,
        EventStateSplitRight<Self, L, R>,
    )
    where
        Self: Sized,
    {
        unbounded(self)
    }
}
