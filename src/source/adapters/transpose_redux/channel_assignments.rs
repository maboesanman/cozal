use std::collections::{hash_map, BTreeMap, BTreeSet, HashMap, HashSet};
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Weak;
use std::task::{Context, Poll, Waker};

use futures_core::Future;

use super::input_buffer::InputBuffer;
use super::output_buffer::OutputBuffer;
use super::steps::{StepWrapper, Steps};
use super::storage::TransposeStorage;
// use crate::source::adapters::transpose_redux::transpose_step_metadata::unwrap_repeat_saturating;
use crate::source::source_poll::{SourcePollInner, SourcePollOk};
use crate::source::traits::SourceContext;
use crate::transposer::step::{Interpolation, StepPoll};
use crate::transposer::Transposer;
use crate::util::replace_waker::ReplaceWaker;
use crate::util::stack_waker::StackWaker;

// manage the association of source and caller channels.
// own steps.
// own interpolations.
// own one_channel_wakers.
struct TransposerInner<T: Transposer> {
    // the collection of steps.
    steps: Steps<T>,

    // These are all the source channels currently in use.
    blocked_source_channels: BTreeSet</* source_channel */ usize>,

    // These are all the currently pending operations, from the perspective of the caller.
    // They can be blocked due to pending source_state, step_future, or interpolation_future.
    blocked_caller_channels:
        HashMap</* caller_channel */ usize, CallerChannelBlockedReason<T>>,

    // these are the currently blocked repeat steps, including the stack wakers
    repeat_step_blocked_reasons: HashMap</* step_id */ usize, StepBlockedReason>,

    // this is the currently blocked original step if it exists, including the stack wakers
    original_step_blocked_reasons: Option<StepBlockedReason>,
}

pub enum StepBlockedReason {
    SourceState {
        source_channel: usize,
        stack_waker:    Weak<StackWaker>,
    },
    Future {
        stack_waker: Weak<StackWaker>,
    },
}

pub enum CallerChannelBlockedReason<T: Transposer> {
    OriginalStep,
    RepeatStep {
        step_id: usize,
    },
    InterpolationSourceState {
        source_channel: usize,
        interpolation:  Interpolation<T, TransposeStorage>,
        forget:         bool,
    },
    InterpolationFuture {
        interpolation: Interpolation<T, TransposeStorage>,
        forget:        bool,
    },
}

struct RepeatStepBlockedCaller {
    source_channel: usize,
    step_id:        usize,
    stack_waker:    Weak<StackWaker>,
}

struct InterpolationWrapper<T: Transposer> {
    source_channel: usize,
    interpolation:  Interpolation<T, TransposeStorage>,
}

enum InnerPoll<T: Transposer> {
    Pending,
    Poll {
        time:              T::Time,
        source_channel:    usize,
        one_channel_waker: Waker,
        state_handler:     StateHandler<T>,
        forget:            bool,
    },
    IngestInputs {
        intput_handler: InputHandler<T>,
    },
    Ready(SourcePollOk<T::Time, T::Output, T::OutputState>),
}

struct StateHandler<T: Transposer> {
    phantom: PhantomData<fn(Self, T::OutputState)>,
}

impl<T: Transposer> StateHandler<T> {
    pub fn handle_state(self, state: Option<T::OutputState>) -> InnerPoll<T> {
        // basically just insert state, remove blocker, and poll again.
        todo!()
    }
}

struct InputHandler<T: Transposer> {
    phantom: PhantomData<fn(Self, T::OutputState)>,
}

impl<T: Transposer> InputHandler<T> {
    pub fn handle_input(self, next_input: Option<T::Input>) -> InnerPoll<T> {
        todo!()
    }
}

impl<T: Transposer> TransposerInner<T> {
    pub fn poll(
        &mut self,
        time: T::Time,
        caller_channel: usize,
        waker: Waker,
        forget: bool,
    ) -> InnerPoll<T> {
        /*
            Procedure:
            get current blocked reason for caller
                if none, find step, start interpolating
                    if you hit any blocker, put record in blocked_caller_channels, as well as the other maps
                    when completing an original_step, emit an InnerPoll::IngestInput
                    if you don't get blocked, just return stuff.

                if source state blocked
                    return an InnerPoll::Poll request

                if future blocked
                    poll relevant future (step or interpolation)
                        pending => return pending
                        ready => remove blocker, return ready
                        needs_state => return InnerPoll::Poll
        */
        match self.blocked_caller_channels.entry(caller_channel) {
            hash_map::Entry::Occupied(e) => Self::handle_blocked_channel(
                e,
                &mut self.repeat_step_blocked_reasons,
                &mut self.original_step_blocked_reasons,
                time,
                waker,
                forget,
                &mut self.steps,
            ),
            hash_map::Entry::Vacant(e) => Self::handle_unblocked_channel(
                e,
                time,
                caller_channel,
                waker,
                forget,
                &mut self.steps,
            ),
        }
    }

    fn find_free_channel() {}

    // emit pending if future blocked, poll if state blocked, and ready if interpolation is done.
    // the state handler will set the blockers up, or continue with handle_unblocked if possible.
    fn handle_blocked_channel(
        mut blocker_entry: hash_map::OccupiedEntry<usize, CallerChannelBlockedReason<T>>,
        repeat_step_blocked_reasons: &mut HashMap</* step_id */ usize, StepBlockedReason>,
        original_step_blocked_reasons: &mut Option<StepBlockedReason>,
        time: T::Time,
        waker: Waker,
        forget: bool,
        steps: &mut Steps<T>,
    ) -> InnerPoll<T> {
        match blocker_entry.get_mut() {
            CallerChannelBlockedReason::OriginalStep => todo!(),
            CallerChannelBlockedReason::RepeatStep {
                step_id,
            } => {
                let step_id = *step_id;
                let mut step_blocked_reason = match repeat_step_blocked_reasons.entry(step_id) {
                    hash_map::Entry::Occupied(e) => e,
                    _ => unreachable!(),
                };
                match step_blocked_reason.get_mut() {
                    StepBlockedReason::SourceState {
                        source_channel,
                        stack_waker,
                    } => {
                        let waker = StackWaker::register(stack_waker, *blocker_entry.key(), waker);
                        match waker {
                            Some(waker) => InnerPoll::Poll {
                                time,
                                source_channel: *source_channel,
                                one_channel_waker: waker,
                                state_handler: StateHandler {
                                    phantom: PhantomData,
                                },
                                forget: false,
                            },
                            None => InnerPoll::Pending,
                        }
                    },
                    StepBlockedReason::Future {
                        stack_waker,
                    } => {
                        let waker = StackWaker::register(stack_waker, *blocker_entry.key(), waker);
                        let step = steps.get_mut_by_sequence_number(step_id).unwrap();
                        match waker {
                            Some(waker) => {
                                let StepPoll {
                                    result,
                                    outputs,
                                } = step.step.poll(waker).unwrap();

                                // this is a repeat step, so no outputs.
                                debug_assert!(outputs.is_empty());
                                drop(outputs);

                                match result {
                                    crate::transposer::step::StepPollResult::NeedsState => {
                                        // find free source_channel, store it in state_handler,
                                        // and store it via state_handler if it returns pending.
                                        InnerPoll::Poll {
                                            time,
                                            source_channel: todo!(),
                                            one_channel_waker: waker,
                                            state_handler: StateHandler {
                                                phantom: PhantomData,
                                            },
                                            forget: false,
                                        }
                                    },
                                    crate::transposer::step::StepPollResult::Pending => {
                                        InnerPoll::Pending
                                    },
                                    crate::transposer::step::StepPollResult::Ready => {
                                        blocker_entry.remove_entry();
                                        // handle_unblocked_channel()
                                        todo!()
                                    },
                                }
                            },
                            None => InnerPoll::Pending,
                        }
                    },
                }
            },
            CallerChannelBlockedReason::InterpolationSourceState {
                source_channel,
                interpolation,
                forget: prev_forget,
            } => {
                *prev_forget = *prev_forget && forget;
                InnerPoll::Poll {
                    time,
                    source_channel: *source_channel,
                    one_channel_waker: waker,
                    state_handler: StateHandler {
                        phantom: PhantomData,
                    },
                    forget: *prev_forget,
                }
            },
            CallerChannelBlockedReason::InterpolationFuture {
                interpolation,
                forget: prev_forget,
            } => {
                let mut cx = Context::from_waker(&waker);
                let mut pin = Pin::new(interpolation);
                let poll = pin.as_mut().poll(&mut cx);
                let interpolation = Pin::into_inner(pin);

                match poll {
                    Poll::Ready(output_state) => {
                        blocker_entry.remove();
                        InnerPoll::Ready(SourcePollOk::Ready(output_state))
                    },
                    Poll::Pending => {
                        *prev_forget = *prev_forget && forget;
                        // find free source_channel, store it in state_handler,
                        // and store it via state_handler if it returns pending.
                        if interpolation.needs_state() {
                            InnerPoll::Poll {
                                time,
                                source_channel: todo!(),
                                one_channel_waker: waker,
                                state_handler: StateHandler {
                                    phantom: PhantomData,
                                },
                                forget: *prev_forget,
                            }
                        } else {
                            InnerPoll::Pending
                        }
                    },
                }
            },
        }
    }

    fn handle_unblocked_channel(
        blocker_entry: hash_map::VacantEntry<usize, CallerChannelBlockedReason<T>>,
        time: T::Time,
        caller_channel: usize,
        waker: Waker,
        forget: bool,
        steps: &mut Steps<T>,
    ) -> InnerPoll<T> {
        // steps.get_last_saturating_before_or_at(time)
        todo!()
    }
}
