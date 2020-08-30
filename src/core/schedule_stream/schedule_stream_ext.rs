use super::realtime_stream::RealtimeStream;
use super::schedule_stream::ScheduleStream;
use super::timestamp::Timestamp;

impl<T> ScheduleStreamExt for T
where
    T: ScheduleStream + Sized,
    <Self as ScheduleStream>::Time: Timestamp,
{
}

/// An extension trait for `ScheduleStream`s that provides the `to_realtime` method only (for now).
pub trait ScheduleStreamExt: ScheduleStream + Sized
where
    <Self as ScheduleStream>::Time: Timestamp,
{
    /// Adapter for converting a schedule_stream into one that yields the items in realtime.
    ///
    /// a reference must be given for the conversion from `T::Time` to `Instant`.
    /// for example if you use `Duration` as your time, a "start time" must be given
    /// so that the duration can be added to something.
    ///
    /// if your timestamp is an `Instant`, then your reference is of type `()` because instants
    /// are already realtime and need no reference.
    fn to_realtime(self, reference: <Self::Time as Timestamp>::Reference) -> RealtimeStream<Self> {
        RealtimeStream::new(self, reference)
    }
}
