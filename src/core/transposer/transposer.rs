use super::context::{InitContext, UpdateContext};
use crate::core::{event::RollbackPayload, Event};
use async_trait::async_trait;

pub type InputEvent<T> = Event<<T as Transposer>::Time, RollbackPayload<<T as Transposer>::Input>>;
pub type ScheduledEvent<T> = Event<<T as Transposer>::Time, <T as Transposer>::Scheduled>;
pub type OutputEvent<T> =
    Event<<T as Transposer>::Time, RollbackPayload<<T as Transposer>::Output>>;
pub(super) type InternalOutputEvent<T> = Event<<T as Transposer>::Time, <T as Transposer>::Output>;

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
    type Input: Unpin + Send + Sync;

    /// The type of the payloads of scheduled events
    ///
    /// the events in the schedule are all of type `Event<Self::Time, Self::Internal>`
    type Scheduled: Unpin + Send + Sync;

    /// The type of the output payloads.
    ///
    /// The input events are of type `Event<Self::Time, RollbackPayload<Self::External>>`
    ///
    /// If a rollback must occur which invalidates previously yielded events, an event of type
    /// `Event<Self::Time, RollbackPayload::Rollback>` will be emitted.
    type Output: Unpin + Send + Sync;

    /// The function to initialize your transposer's events.
    ///
    /// You should initialize your transposer like any other struct.
    /// This function is for initializing the schedule events and emitting any
    /// output events that correspond with your transposer starting.
    ///
    /// `cx` is a context object for performing additional operations.
    /// For more information on `cx` see the [`TransposerContext`] documentation.
    async fn init_events(&mut self, cx: &InitContext<Self>);

    /// The function to respond to input.
    ///
    /// `cx` is a context object for performing additional operations.
    /// For more information on `cx` see the [`TransposerContext`] documentation.
    ///
    /// `inputs` is the collection of payloads of input events that occurred at time `time`.
    /// this is a collection and not one by one because cozal cannot disambiguate
    /// the order of input events whose times are equal, so we need the implementer
    /// to provide an implementation that does not depend on the order of the events.
    /// this is why a `HashSet` is used.
    async fn handle_input(
        &mut self,
        time: Self::Time,
        inputs: &[Self::Input],
        cx: &UpdateContext<Self>,
    );

    /// The function to update your transposer.
    ///
    /// `cx` is a context object for performing additional operations.
    /// For more information on `cx` see the [`TransposerContext`] documentation.
    ///
    /// `events` is the collection of input events that occurred at time `time`.
    /// this is a collection and not one by one because cozal cannot disambiguate
    /// the order of input events whose times are equal, so we need the implementer
    /// to provide an implementation that does not depend on the order of the events.
    /// this is why a `HashSet` is used.
    async fn handle_scheduled(
        &mut self,
        time: Self::Time,
        payload: &Self::Scheduled,
        cx: &UpdateContext<Self>,
    );

    /// Filter out events you know you can't do anything with.
    /// This reduces the amount of events you have to remember for rollback to work
    fn can_handle(_event: &InputEvent<Self>) -> bool {
        true
    }
}
