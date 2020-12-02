use super::SchedulePoll;
use std::pin::Pin;
use std::task::Context;

/// A modified stream that allows for 'scheduling' events.
pub trait StatefulScheduleStream {
    /// The time used to compare.
    type Time: Ord + Copy;
    /// Values yielded by the stream.
    type Item;
    /// The state for the given time.
    ///
    /// This will usually be a smart pointer to something but this is left to the implementer.
    /// Reading of the state could possibly have implications for whether or not a rollback event
    /// must be issued, as well as what data needs to be saved for a replay
    /// (every point queried at runtime must be recorded for deterministic playback).
    /// Because of this, a smart pointer may be used which can note whether or not the state is actually dereferenced,
    /// and therefore how far back rollback events must be issued downstream and how much data to save.
    type State;

    /// Attempt to pull out the next value of this stream, registering the
    /// current task for wakeup if the value is not yet available, and returning
    /// `None` if the stream is exhausted.
    ///
    /// # Return value
    ///
    /// There are several possible return values, each indicating a distinct
    /// stream state:
    ///
    /// - [`SchedulePoll::Ready(val)`](self::SchedulePoll::Ready) means that the stream has successfully
    /// produced a value, `val`, and may produce further values on subsequent
    /// `poll_next` calls.
    ///
    /// - [`SchedulePoll::Scheduled(t)`](self::SchedulePoll::Scheduled) means that the stream's next value is not ready
    /// yet, but there is an event scheduled for time t.
    ///
    /// - [`SchedulePoll::Pending`](self::SchedulePoll::Pending) means that this stream's next value is not ready
    /// yet, and no event is scheduled.
    ///
    /// - [`SchedulePoll::Done`](self::SchedulePoll::Done) means that the stream has terminated, and
    /// [`poll_next`](self::ScheduleStream::poll_next) should not be invoked again.
    ///
    /// # Panics
    ///
    /// Once a stream is finished, i.e. [`SchedulePoll::Done`](self::SchedulePoll::Done) has been returned, further
    /// calls to [`poll_next`](self::ScheduleStream::poll_next) may result in a panic or other "bad behavior".
    fn poll(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: &mut Context<'_>,
    ) -> (Self::State, SchedulePoll<Self::Time, Self::Item>);

    /// Returns the bounds on the remaining length of the stream.
    ///
    /// This is behaves exactly the same as regular streams, and is passed through transparently
    /// in the [`RealtimeStream`](super::realtime_stream::RealtimeStream).
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}
