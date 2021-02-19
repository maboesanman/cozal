use crate::core::{Transposer, transposer::TransposerEngine};

#[cfg(realtime)]
use super::{timestamp::Timestamp, RealtimeStream};
use super::EventStateStream;


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
    fn to_realtime(self, reference: <Self::Time as Timestamp>::Reference) -> RealtimeStream<Self>
        where <Self as EventStateStream>::Time: Timestamp
    {
        RealtimeStream::new(self, reference)
    }

    fn to_engine<T: Transposer<
        Time=Self::Time,
        Input=Self::Event,
        InputState=Self::State,
    >>(self, initial: T) -> TransposerEngine<T, Self>
        where
            T: Clone,
            Self: Sized,
    {
        todo!()
    }
}
