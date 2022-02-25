use core::task::Poll;

/// A modified version of [`futures::task::Poll`], which has two new variants:
/// [`Scheduled`](self::SchedulePoll::Scheduled) and [`Done`](self::SchedulePoll::Done).
pub enum SourcePollOk<T, E, S>
where
    T: Ord + Copy,
{
    /// Indicates all events at or after time T, and all states returned on any channel at or after time T from poll (not poll_forget) should be discarded.
    Rollback(T),

    /// Indicates an unprocessed event is available at or before poll_time.
    Event(E, T),

    /// Indicates no rollback will ever be returned before or at time T.
    Finalize(T),

    /// Indicates all event information up to poll_time is up to date (including if something is scheduled), and returns state.
    /// additionally caller may be woken again until this source is polled at or after time T.
    Scheduled(S, T),

    /// Indicates all event information up to poll_time is up to date (including if something is scheduled), and returns state.
    Ready(S),
}

#[non_exhaustive]
pub enum SourcePollErr<T, Err> {
    OutOfBoundsChannel,
    PollAfterAdvance { advanced: T },
    PollBeforeDefault,
    SpecificError(Err),
}

pub type SourcePoll<T, E, S, Err> = Poll<Result<SourcePollOk<T, E, S>, SourcePollErr<T, Err>>>;
