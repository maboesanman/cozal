use core::pin::Pin;
use std::{cmp::min, collections::BTreeMap, sync::{Arc, Weak}};
use futures::{future::Pending, task::Context};
use pin_project::pin_project;


use crate::core::event_state_stream::{EventStatePoll, EventStateStream};

use super::{engine_time::EngineTime, state_map::StateMap, transposer::Transposer, transposer_frame::TransposerFrame};

/// A struct which implements the [`StatefulScheduleStream`] trait for a [`Transposer`].
///
/// This implementation does the following:
/// - rollback state and replay to resolve instability in the order of the input stream.
/// -- this is useful for online multiplayer games, where the network latency can jumble inputs.
/// - respond to rollback events from the input stream.
/// - record the input events for the purpose of storing replay data.
#[pin_project(project=EngineProjection)]
pub struct TransposerEngine<
    T: Transposer + Clone,
    S: EventStateStream<Time = T::Time, Event = T::Input, State = T::InputState>,
> {
    #[pin]
    input_stream: S,

    input_buffer: BTreeMap<T::Time, Vec<T::Input>>,
    output_buffer: BTreeMap<Arc<EngineTime<T::Time>>, Vec<T::Output>>,
    state_map: StateMap<T, 20>,
}

impl<
    T: Transposer + Clone,
    S: EventStateStream<Time = T::Time, Event = T::Input, State = T::InputState>,
    > TransposerEngine<T, S>
{
    /// create a new TransposerEngine, consuming the input stream.
    pub async fn new(transposer: T, input_stream: S) -> TransposerEngine<T, S> {
        todo!()
    }
}

impl<
    T: Transposer + Clone,
    S: EventStateStream<Time = T::Time, Event = T::Input, State = T::InputState>,
    > EventStateStream for TransposerEngine<T, S>
{
    type Time = T::Time;
    type Event = T::Output;
    type State = T::OutputState;

    fn poll(
        self: Pin<&mut Self>,
        poll_time: Self::Time,
        cx: &mut Context<'_>,
    ) -> EventStatePoll<Self::Time, Self::Event, Self::State> {
        todo!()
    }
}
