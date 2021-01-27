use core::pin::Pin;
use std::{cmp::min, collections::BTreeMap, sync::{Arc, Weak}};
use futures::{future::Pending, task::Context};
use pin_project::pin_project;


use crate::core::event_state_stream::{EventStatePoll, EventStateStream};

use super::{engine_time::EngineTime, transposer::Transposer, transposer_frame::TransposerFrame, transposer_update::{TransposerUpdate, TransposerUpdatePoll}, wrapped_update_result::UpdateResult};

/// A struct which implements the [`StatefulScheduleStream`] trait for a [`Transposer`].
///
/// This implementation does the following:
/// - rollback state and replay to resolve instability in the order of the input stream.
/// -- this is useful for online multiplayer games, where the network latency can jumble inputs.
/// - respond to rollback events from the input stream.
/// - record the input events for the purpose of storing replay data.
#[pin_project(project=EngineProjection)]
pub struct TransposerEngine<
    'a,
    T: Transposer + 'a,
    S: EventStateStream<Time = T::Time, Event = T::Input, State = T::InputState> + Send,
> {
    #[pin]
    input_stream: S,

    #[pin]
    current_update: TransposerUpdate<'a, T>,
    current_state_sender: Option<futures::channel::oneshot::Sender<T::InputState>>,

    input_buffer: BTreeMap<T::Time, Vec<T::Input>>,
    output_buffer: BTreeMap<Arc<EngineTime<T::Time>>, Vec<T::Output>>,
    state_map: BTreeMap<Arc<EngineTime<T::Time>>, StateFrame<T>>,
}

enum StateFrame<T: Transposer> {
    Init{
        transposer_frame: TransposerFrame<T>,
        produced_outputs: bool,
        requested_state: bool,
    },
    Input{
        transposer_frame: TransposerFrame<T>,
        inputs: Vec<T::Input>,
        input_state: Option<T::InputState>,
        produced_outputs: bool,
        requested_state: bool,
    },
    Schedule{
        transposer_frame: TransposerFrame<T>,
        input_state: Option<T::InputState>,
        produced_outputs: bool,
        requested_state: bool,
    },
}

impl<T: Transposer> StateFrame<T> {
    fn produced_outputs(&self) -> bool {
        match self {
            StateFrame::Init{produced_outputs, ..} => *produced_outputs,
            StateFrame::Input{produced_outputs, ..} => *produced_outputs,
            StateFrame::Schedule{produced_outputs, ..} => *produced_outputs,
        }
    }
    fn requested_state(&self) -> bool {
        match self {
            StateFrame::Init{requested_state, ..} => *requested_state,
            StateFrame::Input{requested_state, ..} => *requested_state,
            StateFrame::Schedule{requested_state, ..} => *requested_state,
        }
    }
}

impl<
        'a,
        T: Transposer + Clone + 'a,
        S: EventStateStream<Time = T::Time, Event = T::Input, State = T::InputState> + Send,
    > TransposerEngine<'a, T, S>
{
    /// create a new TransposerEngine, consuming the input stream.
    pub async fn new(transposer: T, input_stream: S) -> TransposerEngine<'a, T, S> {
        Self {
            input_stream,
            current_state_sender: None,
            current_update: TransposerUpdate::new_init(transposer, None),
            input_buffer: BTreeMap::new(),
            output_buffer: BTreeMap::new(),
            state_map: BTreeMap::new(),
        }
    }
}

impl<
        'a,
        T: Transposer + Clone + 'a,
        S: EventStateStream<Time = T::Time, Event = T::Input, State = T::InputState>
            + Unpin
            + Send,
    > EventStateStream for TransposerEngine<'a, T, S>
{
    type Time = T::Time;
    type Event = T::Output;
    type State = T::OutputState;

    fn poll(
        self: Pin<&mut Self>,
        poll_time: Self::Time,
        cx: &mut Context<'_>,
    ) -> EventStatePoll<Self::Time, Self::Event, Self::State> {
        let EngineProjection {
            input_stream,
            current_state_sender,
            current_update,
            input_buffer,
            output_buffer,
            state_map,
        } = self.project();

        // give rust analyzer a little help
        // TODO: DELETE THIS
        let mut input_stream: Pin<&mut S> = input_stream;
        let mut current_update: Pin<&mut TransposerUpdate<'a, T>> = current_update;
        let current_state_sender: &mut Option<futures::channel::oneshot::Sender<T::InputState>> = current_state_sender;
        let input_buffer: &mut BTreeMap<T::Time, Vec<T::Input>> = input_buffer;
        let output_buffer: &mut BTreeMap<Arc<EngineTime<T::Time>>, Vec<T::Output>> = output_buffer;
        let state_map: &mut BTreeMap<Arc<EngineTime<T::Time>>, StateFrame<T>> = state_map;

        loop {
            if current_update.as_mut().is_some() {
                match current_update.as_mut().poll(cx) {
                    TransposerUpdatePoll::Ready {
                        time,
                        inputs,
                        input_state,
                        result,
                    } => {
                        // state_map.insert(time, StateFrame::);
                    }
                    TransposerUpdatePoll::NeedsState(sender) => {
                        *current_state_sender = Some(sender);
                    }
                    TransposerUpdatePoll::Pending => {
                        return EventStatePoll::Pending;
                    }
                }
            }


            match input_stream.as_mut().poll(poll_time, cx) {
                EventStatePoll::Pending => return EventStatePoll::Pending,
                EventStatePoll::Rollback(rollback_time) => {
                    // discard all input_buffer events.
                    // they havent been processed and therefore can't have emitted events.
                    std::mem::drop(input_buffer.split_off(&rollback_time));

                    // strip out the already computed state frames which are at or after rollback_time
                    let post_rollback_frames = state_map.split_off(&EngineTime::Input(rollback_time));

                    // go through the discarded frames, keeping them until you get to something which
                    // relied on information from the source from at or after t.
                    let mut frames_valid = true;
                    for (frame_time, frame) in post_rollback_frames {
                        if frames_valid {
                            match frame_time.as_ref() {
                                EngineTime::Input(input_time) => {
                                    if frame.produced_outputs() {
                                        return EventStatePoll::Rollback(frame_time.time());
                                    }
                                    frames_valid = false;
                                },
                                _ => {
                                    if frame.requested_state() {
                                        if frame.produced_outputs() {
                                            return EventStatePoll::Rollback(frame_time.time());
                                        }
                                        frames_valid = false;
                                    } else {
                                        state_map.insert(frame_time, frame);
                                    }
                                },
                            }
                        } else {
                            if frame.produced_outputs() {
                                return EventStatePoll::Rollback(frame_time.time());
                            }
                        }
                    }
                },
                EventStatePoll::Event(input_time, input) => {
                    if !T::can_handle(input_time, &input) {
                        continue;
                    }


                },
                EventStatePoll::Scheduled(next_event_time, s) => break (s, Some(next_event_time)),
                EventStatePoll::Ready(s) => break (s, None),
                EventStatePoll::Done(s) => break (s, None),
            }
        };
        todo!()
    }
}
