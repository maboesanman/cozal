/// A modified version of [`futures::task::Poll`], which has two new variants:
/// [`Scheduled`](self::SchedulePoll::Scheduled) and [`Done`](self::SchedulePoll::Done).
pub enum EventStatePoll<T, E, S>
where
    T: Ord + Copy,
{
    /// Represents that a value is not ready yet.
    ///
    /// When a function returns `Pending`, the function *must* also
    /// ensure that the current task is scheduled to be awoken when
    /// progress can be made.
    ///
    /// The promise that progress can be made only applies if polled at the same time.
    Pending,

    /// An event was previously emitted which is now invalid
    ///
    ///
    Rollback(T),

    /// Represents that a value is ready and does not occur after the time polled
    Event(T, E),

    /// Represents that a value is ready, but occurs in the future, so the stream should be polled after time t.
    ///
    /// When a function returns `Scheduled`, the function *may never wake the task*.
    /// the contract is that repeated polling will continue to return scheduled(t) for the same t
    /// until new information becomes available or until poll is called
    /// with a new, greater value of t.
    Scheduled(T, S),

    /// Represents that no events will be emitted unless there are new inputs.
    ///
    /// This is distinct from `Pending` because the the responsibility of being awoken is
    /// pushed to the input stream.
    Ready(S),
}
