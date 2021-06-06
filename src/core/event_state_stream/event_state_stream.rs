use super::EventStatePoll;
use std::pin::Pin;
use std::task::Context;

/// A modified stream that allows for 'scheduling' events.
pub trait EventStateStream {
    /// The time used to compare.
    type Time: Ord + Copy;
    /// Values yielded by the stream.
    type Event: Sized;
    /// The state for the given time.
    ///
    /// This will usually be a smart pointer to something but this is left to the implementer.
    /// Reading of the state could possibly have implications for whether or not a rollback event
    /// must be issued, as well as what data needs to be saved for a replay
    /// (every point queried at runtime must be recorded for deterministic playback).
    /// Because of this, a smart pointer may be used which can note whether or not the state is actually dereferenced,
    /// and therefore how far back rollback events must be issued downstream and how much data to save.
    type State: Sized;

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
    ) -> EventStatePoll<Self::Time, Self::Event, Self::State>;

    /// This is an optional optimization. If you don't plan on using the state,
    /// you can call this and the implementer may skip the work to do so if they want.
    ///
    /// By default this simply calls poll and drops the state, returning `()` instead.
    ///
    /// if you do not need to use the state, this should be preferred over poll.
    /// for example, if you are simply verifying the stream does not have new events before a time t,
    /// poll_ignore_state could be faster than poll (with a custom implementation).
    fn poll_events(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: &mut Context<'_>,
    ) -> EventStatePoll<Self::Time, Self::Event, ()> {
        match self.poll(time, cx) {
            EventStatePoll::Pending => EventStatePoll::Pending,
            EventStatePoll::Rollback(t) => EventStatePoll::Rollback(t),
            EventStatePoll::Event(t, e) => EventStatePoll::Event(t, e),
            EventStatePoll::Scheduled(t, _) => EventStatePoll::Scheduled(t, ()),
            EventStatePoll::Ready(_) => EventStatePoll::Ready(()),
        }
    }
}

pub trait EventStateStreamPredictor<S: EventStateStream + ?Sized> {
    fn predict(
        &self,
        base_time: S::Time,
        base_state: S::State,
        target_time: S::Time
    ) -> Option<(S::State, Box<[(S::Time, S::Event)]>)>;
}

impl<S: EventStateStream + ?Sized> EventStateStreamPredictor<S> for () {
    fn predict(&self, _: S::Time, _: S::State, _: S::Time) -> Option<(S::State, Box<[(S::Time, S::Event)]>)> {
        None
    }
}
