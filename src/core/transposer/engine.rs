use core::ops::Deref;
use core::pin::Pin;
use futures::task::Context;
use pin_project::pin_project;
use std::cmp::min;

use crate::{
    core::event::RollbackPayload,
    core::schedule_stream::{SchedulePoll, StatefulScheduleStream},
    core::Event,
};

use super::{engine_internal::{InputStreamItem, TransposerEngineInternal}, transposer::Transposer, transposer_frame::TransposerFrame, transposer_update::TransposerUpdate};

/// A struct which implements the [`StatefulScheduleStream`] trait for a [`Transposer`].
///
/// This implementation does the following:
/// - rollback state and replay to resolve instability in the order of the input stream.
/// -- this is useful for online multiplayer games, where the network latency can jumble inputs.
/// - respond to rollback events from the input stream.
/// - record the input events for the purpose of storing replay data.
#[pin_project]
pub struct TransposerEngine<
    'a,
    T: Transposer + 'a,
    S: StatefulScheduleStream<Time = T::Time, Item = InputStreamItem<T>, State = T::InputState>
        + Send,
> {
    #[pin]
    input_stream: S,

    state: EngineState<'a, T>
}

impl<
        'a,
        T: Transposer + 'a,
        S: StatefulScheduleStream<Time = T::Time, Item = InputStreamItem<T>, State = T::InputState>
            + Send,
    > TransposerEngine<'a, T, S>
{
    /// create a new TransposerEngine, consuming the input stream.
    pub async fn new(transposer: T, input_stream: S) -> TransposerEngine<'a, T, S> {
        // TransposerEngine {
        //     input_stream: input_stream,
        //     internal: TransposerEngineInternal::new(transposer).await,
        // }
        todo!()
    }
}

impl<
        'a,
        T: Transposer + 'a,
        S: StatefulScheduleStream<Time = T::Time, Item = InputStreamItem<T>, State = T::InputState>
            + Unpin
            + Send,
    > StatefulScheduleStream for TransposerEngine<'a, T, S>
{
    type Time = T::Time;
    type Item = Event<T::Time, RollbackPayload<T::Output>>;
    type State = T;

    fn poll(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: &mut Context<'_>,
    ) -> (Self::State, SchedulePoll<Self::Time, Self::Item>) {
        // let projection = self.project();
        // let mut input_stream = projection.input_stream;
        // let internal = projection.internal;

        // // insert events until there are no more input events ready, then poll internal
        // loop {
        //     match input_stream.as_mut().poll(time, cx) {
        //         (in_state, SchedulePoll::Ready(event)) => internal.insert(event, in_state),
        //         (in_state, SchedulePoll::Scheduled(next_input_time)) => {
        //             break match internal.poll(next_input_time, in_state, cx) {
        //                 // If the input stream is scheduled and we are scheduled, then we need to be woken at whichever is earliest.
        //                 (out_state, SchedulePoll::Scheduled(next_output_time)) => {
        //                     let next_time = min(next_input_time, next_output_time);
        //                     (out_state, SchedulePoll::Scheduled(next_time))
        //                 }
        //                 // if the input stream is scheduled and we are pending then we need to be woken at the input scheduled time.
        //                 (out_state, SchedulePoll::Pending) => {
        //                     (out_state, SchedulePoll::Scheduled(next_input_time))
        //                 }
        //                 // ready and done are both fine to return as is.
        //                 poll => poll,
        //             };
        //         }
        //         (in_state, SchedulePoll::Pending) => break internal.poll(time, in_state, cx),
        //         (in_state, SchedulePoll::Done) => break internal.poll(time, in_state, cx),
        //     }
        // }
        todo!()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // self.internal.size_hint()
        todo!()
    }
}

enum EngineState<'a, T: Transposer> {
    Waiting(TransposerFrame<T>),
    Updating(TransposerUpdate<'a, T>),
}