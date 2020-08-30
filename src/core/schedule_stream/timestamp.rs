use std::time::Duration;
use std::time::Instant;

/// Timestamps are the types used by `ScheduleStreams` to allow for streams that need a
/// more complex timestamp type.
///
/// for example, time could be quantized into "ticks", or could incorporate an added "priority level".
/// all these types would need to do is implement timestamp to go to and from `Instants`
pub trait Timestamp: Ord + Copy {
    /// Reference is usually going to be either `Instant` or `()`, depending on if your implementing type
    /// is relative or absolute.
    ///
    /// Essentially, this is the type of the "start time" that this type requires to calculate a time.
    type Reference;

    /// Compute an instant from a timestamp and a reference point.
    ///
    /// this function should generally "agree" with `get_timestamp`.
    ///
    /// in particular ```
    /// let timestamp_1: Timestamp;
    /// let instant_1 = timestamp.get_instant(ref);
    /// let timestamp2 = instant_1.get_timestamp(ref);
    /// assert_eq!(timestamp_1, timestamp_2);
    /// ```
    fn get_instant(&self, reference: &Self::Reference) -> Instant;

    /// Compute a timestamp from an instant and a reference point.
    ///
    /// this function should generally "agree" with `get_instant`.
    ///
    /// in particular ```
    /// let timestamp_1: Timestamp;
    /// let instant_1 = timestamp.get_instant(ref);
    /// let timestamp2 = instant_1.get_timestamp(ref);
    /// assert_eq!(timestamp_1, timestamp_2);
    /// ```
    fn get_timestamp(instant: &Instant, reference: &Self::Reference) -> Self;
}

impl Timestamp for Instant {
    // Instants are absolute, so don't need a point of reference.
    type Reference = ();

    fn get_instant(&self, _reference: &Self::Reference) -> Instant {
        *self
    }
    fn get_timestamp(instant: &Instant, _reference: &Self::Reference) -> Self {
        *instant
    }
}

impl Timestamp for Duration {
    // Duration needs a "since" to create a realtime value.
    type Reference = Instant;

    fn get_instant(&self, reference: &Self::Reference) -> Instant {
        *reference + *self
    }

    fn get_timestamp(instant: &Instant, reference: &Self::Reference) -> Self {
        *instant - *reference
    }
}
