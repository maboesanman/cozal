use async_trait::async_trait;
use context::{HandleInputContext, HandleScheduleContext, InitContext, InterpolateContext};

pub mod context;
// mod evaluate_to;
mod expire_handle;
mod sequence_frame;
mod test;

pub use expire_handle::ExpireHandle;

/// A `Transposer` is a type that can update itself in response to events.
///
/// the purpose of this type is to provide an abstraction for game logic which can be used to add rollback and
/// realtime event scheduling, replays, and possibly more.
///
/// it is *heavily* recommended to use immutable structure sharing data types (for example, the [`im`] crate)
/// in the implementing struct, because [`clone`](Clone::clone) is called often and should be a cheap operation.
///
/// The name comes from the idea that we are converting a stream of events into another stream of events,
/// perhaps in the way a stream of music notes can be *transposed* into another stream of music notes.
#[async_trait(?Send)]
pub trait Transposer {
    /// The type used as the 'time' for events. This must be Ord and Copy because it is frequently used for comparisons,
    /// and it must be [`Default`] because the default value is used for the timestamp of events emitted.
    /// by the init function.
    type Time: Copy + Ord + Default;

    type InputState: Unpin;

    type OutputState;

    /// The type of the input payloads.
    ///
    /// The input events are of type `Event<Self::Time, RollbackPayload<Self::Input>>`
    ///
    /// This type is not intended to contain timing information. It may if you need it, but
    /// no timing information contained inside your `Input` type will be used to inform the order
    /// that events are handled.
    type Input;

    /// The type of the payloads of scheduled events
    ///
    /// the events in the schedule are all of type `Event<Self::Time, Self::Scheduled>`
    type Scheduled: Clone;

    /// The type of the output payloads.
    ///
    /// The output events are of type `Event<Self::Time, RollbackPayload<Self::Output>>`
    ///
    /// If a rollback must occur which invalidates previously yielded events, an event of type
    /// `Event<Self::Time, RollbackPayload::Rollback>` will be emitted.
    type Output;

    /// The function to initialize your transposer's events.
    ///
    /// You should initialize your transposer like any other struct.
    /// This function is for initializing the schedule events and emitting any
    /// output events that correspond with your transposer starting.
    ///
    /// `cx` is a context object for performing additional operations.
    /// For more information on `cx` see the [`InitContext`] documentation.
    async fn init(&mut self, _cx: &mut dyn InitContext<Self>) {}

    /// The function to respond to input.
    ///
    /// `inputs` is the collection of payloads of input events that occurred at time `time`.
    /// this is a collection and not one by one because cozal cannot disambiguate
    /// the order of input events whose times are equal, so we need the implementer
    /// to provide an implementation that does not depend on the order of the events.
    ///
    /// `cx` is a context object for performing additional operations like scheduling events.
    /// For more information on `cx` see the [`UpdateContext`] documentation.
    async fn handle_input(
        &mut self,
        _time: Self::Time,
        _inputs: &[Self::Input],
        _cx: &mut dyn HandleInputContext<Self>,
    ) {
    }

    /// The function to respond to internally scheduled events.
    ///
    /// `time` and `payload` correspond with the event to be handled.
    ///
    /// `cx` is a context object for performing additional operations like scheduling events.
    /// For more information on `cx` see the [`UpdateContext`] documentation.
    async fn handle_scheduled(
        &mut self,
        _time: Self::Time,
        _payload: Self::Scheduled,
        _cx: &mut dyn HandleScheduleContext<Self>,
    ) {
    }

    /// The function to interpolate between states
    ///
    /// handle_input and handle_scheduled only operate on discrete times.
    /// If you want the state between two of these times, you have to calculate it.
    ///
    /// `base_time` is the time of the `self` parameter
    /// `interpolated_time` is the time being requested `self`
    /// `cx is a context object for performing additional operations like requesting state.
    async fn interpolate(
        &self,
        base_time: Self::Time,
        interpolated_time: Self::Time,
        cx: &mut dyn InterpolateContext<Self>,
    ) -> Self::OutputState;

    /// Filter out events you know you can't do anything with.
    /// This reduces the amount of events you have to remember for rollback to work
    fn can_handle(_time: Self::Time, _input: &Self::Input) -> bool {
        true
    }
}
