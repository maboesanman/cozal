/// A modified version of [`futures::task::Poll`], which has two new variants:
/// [`Scheduled`](self::SchedulePoll::Scheduled) and [`Done`](self::SchedulePoll::Done).
pub enum StatefulSchedulePoll<T, P, S>
where
    T: Ord + Copy,
{
    /// Represents that a value is ready and does not occur after the time polled
    Ready(T, P, S),

    /// Represents that a value is ready, but occurs in the future, so the stream should be polled after time t.
    ///
    /// When a function returns `Scheduled`, the function *may never wake the task*.
    /// the contract is that repeated polling will continue to return scheduled(t) for the same t
    /// until new information becomes availavle (via the input stream) or until poll is called
    /// with a new, greater value of t.
    Scheduled(T, S),

    Waiting(S),

    /// Represents that a value is not ready yet.
    ///
    /// When a function returns `Pending`, the function *must* also
    /// ensure that the current task is scheduled to be awoken when
    /// progress can be made.
    Pending,

    /// Represents the end of the stream.
    Done(S),
}
