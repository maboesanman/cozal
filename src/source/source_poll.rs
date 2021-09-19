use core::task::Poll;

/// A modified version of [`futures::task::Poll`], which has two new variants:
/// [`Scheduled`](self::SchedulePoll::Scheduled) and [`Done`](self::SchedulePoll::Done).
pub enum SourcePollOk<T, E, S>
where
    T: Ord + Copy,
{
    /// Indicates all events at or after time T, and all states returned from poll (not poll_forget
    /// should be discarded.
    ///
    Rollback(T),

    /// Represents that a value is ready and does not occur after the time polled.
    Event(E, T),

    /// Indicates no rollback will ever be returned before or at time T.
    Finalize(T),

    /// Represents that a value is ready, but occurs in the future, so the stream should be polled after time t.
    ///
    /// When a function returns `Scheduled`, the function *may never wake the task*.
    /// the contract is that repeated polling will continue to return scheduled(t) for the same t
    /// until new information becomes available or until poll is called
    /// with a new, greater value of t.
    Scheduled(S, T),

    /// Represents that no events will be emitted unless there are new inputs.
    ///
    /// This is distinct from `Pending` because the the responsibility of being awoken is
    /// pushed to the input stream.
    Ready(S),
}

pub enum SourcePollErr<T, Err> {
    OutOfBoundsChannel,
    PollAfterAdvance { advanced: T },
    SpecificError(Err),
}

pub enum AdvanceErr {}

pub type SourcePoll<T, E, S, Err> = Poll<Result<SourcePollOk<T, E, S>, SourcePollErr<T, Err>>>;
pub type SourceAdvance = Result<(), AdvanceErr>;
