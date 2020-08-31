use super::transposer_context::TransposerContext;
use super::{transposer_event::TransposerEvent, transposer_expire_handle::ExpireHandle};
use crate::core::event::event::Event;
use async_trait::async_trait;

/// The result of the init function for a [`Transposer`].
pub struct InitResult<T: Transposer> {
    /// The initial value of your transposer.
    pub transposer: T,

    /// Events to initialize the schedule with.
    pub new_events: Vec<Event<T::Time, T::Internal>>,

    /// New events to yield downstream
    ///
    /// the time is not specified here, because events are always emitted exactly when they are created.
    /// In the case of the [`InitResult`], they are emitted with `T::Time.default()`.
    ///
    /// If you need to emit an event in the future, schedule an internal event that, when handled, emits an output event.
    pub emitted_events: Vec<T::Out>,
}

/// The result of the update function for a [`Transposer`].
pub struct UpdateResult<T: Transposer> {
    /// If you have made changes to your state, you can set the state here.
    ///
    /// A value of [`None`] means that you intend to keep the previous value.
    pub new_transposer: Option<T>,

    /// A [`Vec`] of expire handles.
    pub expired_events: Vec<ExpireHandle>,

    /// New events to schedule. The order events are placed here is important,
    /// as the expiration handles are created by pointing to a specific index in the
    /// new events array.
    pub new_events: Vec<Event<T::Time, T::Internal>>,

    /// New events to yield downstream
    ///
    /// the time is not specified here, because events are always emitted exactly when they are created.
    ///
    /// If you need to emit an event in the future, schedule an internal event that, when handled, emits an output event.
    pub emitted_events: Vec<T::Out>,

    /// Whether or not the stream should yield [`Done`](crate::core::schedule_stream::schedule_stream::SchedulePoll::Done)
    /// and terminate.
    pub exit: bool,
}

/// A `Transposer` is a type that can create an updated version of itself in response to events.
///
/// the purpose of this type is to provide an abstraction which can be used to add rollback and
/// realtime event scheduling, replays, and possibly more
///
/// it is *heavily* recommended to use immutable structure sharing data types (for example, the [`im`] crate)
/// in the implementing struct, because unless you store no state, you will likely need to call
/// [`clone`](Clone::clone) inside your implementation of update, which is called every time
/// an event occurs.
///
/// The name comes from the idea that we are converting a stream of events into another stream of events,
/// perhaps in the way a stream of music notes can be *transposed* into another stream of music notes.
#[async_trait]
pub trait Transposer: Clone + Unpin + Send + Sync {
    /// The type used as the 'time' for events. This must be Ord and Copy because it is frequently used for comparisons,
    /// and it must be [`Default`] because the default value is used for the timestamp of events emitted.
    /// by the init function.
    type Time: Copy + Ord + Default + Send + Sync;

    /// The type of the input payloads.
    ///
    /// The input events are of type `Event<Self::Time, RollbackPayload<Self::External>>`
    ///
    /// This type is not intended to contain timing information. It may if you need it, but
    /// no timing information contained inside your `External` type will be used to inform the order
    /// that events are handled.
    type External: Unpin + Send + Sync;

    /// The type of the payloads of scheduled events
    ///
    /// the events in the schedule are all of type `Event<Self::Time, Self::Internal>`
    type Internal: Unpin + Send + Sync;

    /// The type of the output payloads.
    ///
    /// The input events are of type `Event<Self::Time, RollbackPayload<Self::External>>`
    ///
    /// If a rollback must occur which invalidates previously yielded events, an event of type
    /// `Event<Self::Time, RollbackPayload::Rollback>` will be emitted.
    type Out: Unpin + Send + Sync;

    /// The function to initialize your transposer.
    ///
    /// You must initialize a transposer (passed through the [`transposer`](InitResult::transposer) prop of [`InitResult`])
    ///
    /// `cx` is a context object for performing additional operations.
    /// For more information on `cx` see the [`TransposerContext`] documentation.
    async fn init(cx: &TransposerContext) -> InitResult<Self>;

    /// The function to update your transposer.
    ///
    /// `cx` is a context object for performing additional operations.
    /// For more information on `cx` see the [`TransposerContext`] documentation.
    ///
    /// `events` is the collection of events that this update is meant to respond to.
    /// All events in `events` have **the same time**. You will receive all events
    /// with equal times (according to [`Eq`] as required by [`Ord`]). The events here have deterministic
    /// order **if and only if** no two input events have the same time. If this is causing problems, [`Self::Time`]
    /// should be selected to be a type with a more discerning implementation of [`Ord`]
    /// (perhaps one that has a secondary value to sort on).
    async fn update(
        &self,
        cx: &TransposerContext,
        events: Vec<&TransposerEvent<Self>>,
    ) -> UpdateResult<Self>;

    /// Filter out events you know you can't do anything with.
    /// This reduces the amount of events you have to remember for rollback to work
    fn can_process(_event: &Event<Self::Time, Self::External>) -> bool {
        true
    }
}
