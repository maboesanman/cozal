use core::ops::Deref;
use core::pin::Pin;
use futures::{channel::oneshot::Sender, task::Context};
use pin_project::pin_project;
use std::{cmp::min, collections::{BTreeMap, VecDeque}, mem::MaybeUninit, unimplemented, unreachable};

use crate::{core::Event, core::event::RollbackPayload, core::schedule_stream::{Peekable, StatefulSchedulePoll, StatefulScheduleStream}};

use super::{InitContext, transposer::Transposer, transposer_frame::TransposerFrame, transposer_update::{ReadyResult, TransposerUpdate, TransposerUpdatePoll}, wrapped_update_result::WrappedUpdateResult};

type InputBuffer<T> = BTreeMap<<T as Transposer>::Time, <T as Transposer>::Input>;
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
    T: Transposer + Clone + 'a,
    S: StatefulScheduleStream<Time = T::Time, Item = T::Input, State = T::InputState>
        + Send,
> {
    #[pin]
    input_stream: Peekable<S>,

    #[pin]
    state: EngineState<'a, T>,
}

impl<
        'a,
        T: Transposer + Clone + 'a,
        S: StatefulScheduleStream<Time = T::Time, Item = T::Input, State = T::InputState>
            + Send,
    > TransposerEngine<'a, T, S>
{
    /// create a new TransposerEngine, consuming the input stream.
    pub async fn new(transposer: T, input_stream: S) -> TransposerEngine<'a, T, S> {
        TransposerEngine {
            input_stream: Peekable::new(input_stream),

            state: EngineState::new(transposer).await,
        }
    }
}

impl<
        'a,
        T: Transposer + Clone + 'a,
        S: StatefulScheduleStream<Time = T::Time, Item = T::Input, State = T::InputState>
            + Unpin
            + Send,
    > StatefulScheduleStream for TransposerEngine<'a, T, S>
{
    type Time = T::Time;
    type Item = T::Output;
    type State = T;

    fn poll(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: &mut Context<'_>,
    ) -> StatefulSchedulePoll<Self::Time, Self::Item, Self::State> {
        let EngineProjection {
            mut input_stream,
            mut state,
        } = self.project();

        'poll: loop {
            match state.as_mut().project() {
                EngineStateProjection::Waiting(frame) => {

                    let next_scheduled_time = frame.get_next_schedule_time();
                    let poll_time = match next_scheduled_time {
                        Some(t) => t,
                        None => time,
                    };
                    let next_input_time = match input_stream.as_mut().peek(poll_time, cx) {
                        StatefulSchedulePoll::Ready(t, _, _) => Some(*t),
                        _ => None
                    };
                    

                    // For convenience
                    enum UpdateCase<T> {
                        None,
                        Input(T),
                        Scheduled(T),
                    };

                    // Determine which update type comes next.
                    let mut case = match (next_input_time, next_scheduled_time) {
                        
                        // if there is nothing upcoming in the schedule or input, there's nothing to do.
                        (None, None) => UpdateCase::None,

                        // if there is an input occuring before or equal in time to the scheduled event, process that.
                        (Some(t), _) => UpdateCase::Input(t),

                        // if there is only a scheduled event, process that.
                        (None, Some(t)) => UpdateCase::Scheduled(t),
                    };

                    // If we aren't ready for the next update, switch to None.
                    if let Some(t) = match case {
                        UpdateCase::None => None,
                        UpdateCase::Input(t) => Some(t),
                        UpdateCase::Scheduled(t) => Some(t),
                    } {
                        if time < t {
                            case = UpdateCase::None;
                        }
                    }

                    match case {
                        UpdateCase::None => break 'poll StatefulSchedulePoll::Waiting(frame.transposer.clone()),
                        UpdateCase::Input(next_input_time) => {
                            let mut next_inputs = Vec::new();
                            let mut input_state = MaybeUninit::uninit();
                            'input: loop {
                                match input_stream.as_mut().peek(next_input_time, cx) {
                                    StatefulSchedulePoll::Ready(t, _p, _s) => {
                                        if *t == next_input_time {
                                            if let StatefulSchedulePoll::Ready(_t, p, s) = input_stream.as_mut().poll(next_input_time, cx) {
                                                next_inputs.push(p);
                                                input_state = MaybeUninit::new(s);
                                            } else {
                                                unreachable!()
                                            }
                                        } else {
                                            unreachable!()
                                        }
                                    },
                                    _ => break 'input
                                }
                            };

                            // SAFETY: the first item is guranteed to match, so we definitely have a state.
                            let input_state = unsafe { input_state.assume_init() };

                            // SAFETY: this is safe as long as we don't move anything out of an update.
                            take_mut::take(unsafe { state.as_mut().get_unchecked_mut() }, |current_state| {
                                if let EngineState::Waiting(frame) = current_state {
                                    let update = TransposerUpdate::new_input(frame, next_input_time, next_inputs, input_state);
                                    EngineState::Updating(update, None)
                                } else {
                                    // we already are in this branch from the first match
                                    unreachable!()
                                }
                            });
                        }
                        UpdateCase::Scheduled(_) => {
                            // SAFETY: this is safe as long as we don't move anything out of an update.
                            take_mut::take(unsafe { state.as_mut().get_unchecked_mut() }, |state| {
                                if let EngineState::Waiting(frame) = state {
                                    let update = TransposerUpdate::new_schedule(frame, None);
                                    EngineState::Updating(update, None)
                                } else {
                                    // we already are in this branch from the first match
                                    unreachable!()
                                }
                            });
                        }
                    }

                    if let EngineStateProjection::Updating(update, _) = state.as_mut().project() {
                        update.init_pinned();
                    } else {
                        unreachable!()
                    }

                    // fall through loop, trying to poll update
                }
                EngineStateProjection::Updating(mut update, sender) => {

                    // if we have a sender, poll for something to send it.
                    if let Some(sender) = std::mem::take(sender) {
                        let time = update.as_ref().get_ref().time();
                        let state = match input_stream.as_mut().poll(time, cx) {

                            // this gives us the state in the past, which is not allowed in this configuration.
                            StatefulSchedulePoll::Ready(_, _, _) => {
                                panic!("all inputs at time t must be ready at the same time (no pending in the middle of them)")
                            }

                            // these give us the state for the time we asked for.
                            StatefulSchedulePoll::Scheduled(_, s) => s,
                            StatefulSchedulePoll::Waiting(s) => s,
                            StatefulSchedulePoll::Done(s) => s,

                            // this says the state is not ready; we cannot proceed until it is.
                            StatefulSchedulePoll::Pending => break 'poll StatefulSchedulePoll::Pending,
                        };
                        let _ = sender.send(state);
                    }

                    
                    match update.as_mut().poll(cx) {
                        TransposerUpdatePoll::Ready(result) => {
                            let ReadyResult {
                                result: WrappedUpdateResult {
                                    frame: updated_frame,
                                    outputs,
                                    ..
                                },
                                ..
                            } = result;

                            // SAFETY: this is safe as long as we don't move anything out of an update.
                            let state_mut = unsafe { state.as_mut().get_unchecked_mut() };
                            *state_mut = if outputs.len() == 0 {
                                EngineState::Waiting(updated_frame)
                            } else {
                                EngineState::Emitting(updated_frame, VecDeque::from(outputs))
                            };
                        },
                        TransposerUpdatePoll::NeedsState(new_sender) => {
                            if sender.is_some() {
                                panic!("updater requested second state");
                            }

                            *sender = Some(new_sender);
                        },
                        TransposerUpdatePoll::Pending => break 'poll {
                            StatefulSchedulePoll::Pending
                        },
                    }
                }
                EngineStateProjection::Emitting(frame, output_buffer) => {
                    let output = output_buffer.pop_front();
                    let transposer = frame.transposer.clone();
                    let time = frame.time();
                    if output_buffer.len() == 0 {
                        // SAFETY: this is safe as long as we don't move anything out of an update.
                        take_mut::take(unsafe { state.as_mut().get_unchecked_mut() }, |state| {
                            if let EngineState::Emitting(frame, _) = state {
                                EngineState::Waiting(frame)
                            } else {
                                // we already are in this branch from the first match
                                unreachable!()
                            }
                        });
                    }

                    if let Some(output) = output {
                        break 'poll StatefulSchedulePoll::Ready(time, output, transposer)
                    }
                }
            }
        }
    }
}

#[pin_project(project=EngineStateProjection)]
enum EngineState<'a, T: Transposer + Clone + 'a> {
    // no update is currently in progress. no output events are being emitted.
    Waiting(TransposerFrame<T>),

    // an update is currently in progress.
    Updating(#[pin] TransposerUpdate<'a, T>, Option<Sender<T::InputState>>),

    // an update finished with output events, which are not fully emitted yet.
    Emitting(TransposerFrame<T>, VecDeque<T::Output>),
}

impl<'a, T: Transposer + Clone + 'a> EngineState<'a, T> {
    pub async fn new(transposer: T) -> EngineState<'a, T> {
        let mut frame = TransposerFrame::new(transposer);

        let mut context = InitContext::new();
        T::init_events(&mut frame.transposer, &mut context).await;
        let output_buffer = VecDeque::from(context.outputs);

        if output_buffer.is_empty() {
            Self::Waiting(frame)
        } else {
            Self::Emitting(frame, output_buffer)
        }
    }
}