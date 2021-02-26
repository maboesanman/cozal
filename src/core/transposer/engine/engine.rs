use core::pin::Pin;
use std::collections::BTreeMap;
use futures::{task::Context};
use pin_project::pin_project;


use crate::core::{Transposer, event_state_stream::{EventStatePoll, EventStateStream}};

use super::{engine_time::EngineTime, input_buffer::InputBuffer}; //, state_map::StateMap};

/// A struct which implements the [`StatefulScheduleStream`] trait for a [`Transposer`].
///
/// This implementation does the following:
/// - rollback state and replay to resolve instability in the order of the input stream.
/// -- this is useful for online multiplayer games, where the network latency can jumble inputs.
/// - respond to rollback events from the input stream.
/// - record the input events for the purpose of storing replay data.
#[pin_project(project=EngineProjection)]
pub struct TransposerEngine<
    'transposer,
    T: Transposer + Clone + 'transposer,
    S: EventStateStream<Time = T::Time, Event = T::Input, State = T::InputState>,
>
where T::Scheduled: Clone {
    #[pin]
    input_stream: S,

    input_buffer: InputBuffer<T::Time, T::Input>,
    output_buffer: BTreeMap<EngineTime<'transposer, T::Time>, Vec<T::Output>>,
    // state_map: StateMap<'transposer, T, 20>,
}

impl<
    't,
    T: Transposer + Clone + 't,
    S: EventStateStream<Time = T::Time, Event = T::Input, State = T::InputState>,
    > TransposerEngine<'t, T, S>
    where T::Scheduled: Clone 
{
    /// create a new TransposerEngine, consuming the input stream.
    pub fn new(_input_stream: S, _initial_transposer: T) -> TransposerEngine<'t, T, S> {
        todo!()
    }
}

impl<
    't,
    T: Transposer + Clone + 't,
    S: EventStateStream<Time = T::Time, Event = T::Input, State = T::InputState>,
    > EventStateStream for TransposerEngine<'t, T, S>
    where T::Scheduled: Clone 
{
    type Time = T::Time;
    type Event = T::Output;
    type State = T::OutputState;

    fn poll(
        self: Pin<&mut Self>,
        _poll_time: Self::Time,
        _cx: &mut Context<'_>,
    ) -> EventStatePoll<Self::Time, Self::Event, Self::State> {
        // split the waker here to determine if woken by the input stream or the pending futures in state_map

        todo!()
    }
}
