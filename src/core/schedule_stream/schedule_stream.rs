use super::{StatefulSchedulePoll, StatefulScheduleStream};
use std::pin::Pin;
use std::task::Context;

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
    fn poll_next(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: &mut Context<'_>,
    ) -> StatefulSchedulePoll<Self::Time, Self::Item, ()>;

    /// Returns the bounds on the remaining length of the stream.
    ///
    /// This is behaves exactly the same as regular streams, and is passed through transparently
    /// in the [`RealtimeStream`](super::realtime_stream::RealtimeStream).
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

impl<S> StatefulScheduleStream for S
where
    S: ScheduleStream,
{
    type Item = S::Item;
    type Time = S::Time;
    type State = ();

    fn poll(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: &mut Context<'_>,
    ) -> StatefulSchedulePoll<Self::Time, Self::Item, Self::State> {
        self.poll_next(time, cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self as &S).size_hint()
    }
}
