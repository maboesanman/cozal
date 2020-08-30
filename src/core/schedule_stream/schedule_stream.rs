use super::{realtime_stream::RealtimeStream, timestamp::Timestamp};
use std::pin::Pin;
use std::task::Context;

/// A modified version of futures::task::Poll, which has two new variants: Scheduled and Done.
pub enum SchedulePoll<T, P>
where
    T: Ord + Copy,
{
    /// Represents that a value is ready and does not occur after the time polled
    Ready(P),

    /// Represents that a value is ready, but occurs in the future, so the stream should be polled after time t.
    ///
    /// When a function returns `Scheduled`, the function *may never wake the task*.
    /// the contract is that repeated polling will continue to return scheduled(t) for the same t
    /// until new information becomes availavle (via the input stream) or until poll is called
    /// with a new, greater value of t.
    Scheduled(T),

    /// Represents that a value is not ready yet.
    ///
    /// When a function returns `Pending`, the function *must* also
    /// ensure that the current task is scheduled to be awoken when
    /// progress can be made.
    Pending,

    /// Represents the end of the stream.
    Done,
}

/// A modified stream that allows for 'scheduling' events.
pub trait ScheduleStream {
    /// The time used to compare.
    type Time: Ord + Copy;
    /// Values yielded by the stream.
    type Item;

    /// Attempt to pull out the next value of this stream, registering the
    /// current task for wakeup if the value is not yet available, and returning
    /// `None` if the stream is exhausted.
    ///
    /// # Return value
    ///
    /// There are several possible return values, each indicating a distinct
    /// stream state:
    ///
    /// - `Poll::Pending` means that this stream's next value is not ready
    /// yet, and no event is scheduled.
    ///
    /// - `Poll::Ready(val)` means that the stream has successfully
    /// produced a value, `val`, and may produce further values on subsequent
    /// `poll_next` calls.
    ///
    /// - `Poll::Done` means that the stream has terminated, and
    /// `poll_next` should not be invoked again.
    ///
    /// = `Poll::Scheduled(t)` means that the stream's next value is not ready
    /// yet, but there is an event scheduled for time t.
    ///
    /// # Panics
    ///
    /// Once a stream is finished, i.e. `Done` has been returned, further
    /// calls to `poll_next` may result in a panic or other "bad behavior".
    fn poll_next(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: &mut Context<'_>,
    ) -> SchedulePoll<Self::Time, Self::Item>;
}
