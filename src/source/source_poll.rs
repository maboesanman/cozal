use core::task::Poll;

/// A modified version of [`futures::task::Poll`], which has two new variants:
/// [`Scheduled`](self::SchedulePoll::Scheduled) and [`Done`](self::SchedulePoll::Done).
pub enum SourcePollOk<T, E, S>
where
    T: Ord + Copy,
{
    /// An event was previously emitted which is now invalid
    ///
    ///
    Rollback(T),

    /// Represents that a value is ready and does not occur after the time polled
    Event(E, T),

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

pub enum SourcePollErr<Err> {
    OutOfBoundsChannel,
    SpecificError(Err),
}

pub type SourcePoll<T, E, S, Err> = Poll<Result<SourcePollOk<T, E, S>, SourcePollErr<Err>>>;
