use std::collections::{BTreeMap, HashMap};
use std::pin::Pin;
use std::sync::Weak;
use std::task::{Context, Poll, Waker};

use futures_core::Future;
use transposer::step::{Interpolation, StepPoll};
use transposer::Transposer;
use util::extended_entry::btree_map::{
    get_first_vacant,
    get_occupied as btree_map_get_occupied,
    OccupiedExtEntry as BTreeMapOccupiedEntry,
    VacantExtEntry as BTreeMapVacantEntry,
};
use util::extended_entry::hash_map::{
    get_occupied as hash_map_get_occupied,
    OccupiedExtEntry as HashMapOccupiedEntry,
    VacantExtEntry as HashMapVacantEntry,
};
use util::extended_entry::option::{
    get_occupied as option_get_occupied,
    OccupiedExtEntry as OptionOccupiedEntry,
    VacantExtEntry as OptionVacantEntry,
};
use util::extended_entry::vecdeque::{
    get_ext_entry as vecdeque_get_ext_entry,
    ExtEntry as VecDequeEntry,
};
use util::replace_mut::replace;
use util::replace_waker::ReplaceWaker;
use util::stack_waker::StackWaker;

use super::steps::{StepWrapper, Steps};
use super::storage::TransposeStorage;
// use crate::adapters::transpose_redux::transpose_step_metadata::unwrap_repeat_saturating;
use crate::traits::SourceContext;

// manage the association of source and caller channels.
// own steps.
// own interpolations.
// own one_channel_wakers.
pub struct ChannelStatuses<T: Transposer> {
    // the collection of steps.
    pub steps: Steps<T>,

    // These are all the source channels currently in use.
    // this is just a hack to use the entry api.
    pub blocked_source_channels: BTreeMap</* source_channel */ usize, ()>,

    // These are all the currently pending operations, from the perspective of the caller.
    // They can be blocked due to pending source_state, step_future, or interpolation_future.
    pub blocked_caller_channels:
        HashMap</* caller_channel */ usize, CallerChannelBlockedReason<T>>,

    // these are the currently blocked repeat steps, including the stack wakers
    pub repeat_step_blocked_reasons: HashMap</* step_id */ usize, RepeatStepBlockedReason>,

    // this is the currently blocked original step if it exists, including the stack wakers
    pub original_step_blocked_reasons: Option<OriginalStepBlockedReason>,
}

impl<T: Transposer> ChannelStatuses<T> {
    pub fn new(transposer: T, rng_seed: [u8; 32]) -> Self {
        Self {
            steps: Steps::new(transposer, rng_seed),
            blocked_source_channels: BTreeMap::new(),
            blocked_caller_channels: HashMap::new(),
            repeat_step_blocked_reasons: HashMap::new(),
            original_step_blocked_reasons: None,
        }
    }

    pub fn get_scheduled_time(&self) -> Option<T::Time> {
        let step = self.steps.get_last();

        if step.step.is_original() && !step.step.is_saturated() {
            Some(step.step.raw_time())
        } else {
            None
        }
    }

    /// from the current state, get the status.
    ///
    /// this internally holds mutable refs to the ChannelStatuses
    pub fn get_channel_status(&mut self, caller_channel: usize) -> CallerChannelStatus<'_, T> {
        let ChannelStatuses {
            ref mut blocked_caller_channels,
            ref mut steps,
            ref mut blocked_source_channels,
            ref mut repeat_step_blocked_reasons,
            ref mut original_step_blocked_reasons,
        } = self;

        let caller_channel_entry = hash_map_get_occupied(blocked_caller_channels, caller_channel);

        let mut caller_channel = match caller_channel_entry {
            Err(caller_channel) => {
                return CallerChannelStatus::Free(Free {
                    caller_channel,
                    steps,
                    blocked_source_channels,
                    repeat_step_blocked_reasons,
                    original_step_blocked_reasons,
                })
            },
            Ok(caller_channel) => caller_channel,
        };

        match caller_channel.get_value_mut() {
            CallerChannelBlockedReason::OriginalStep => {
                let step = steps.get_last_entry().unwrap();
                let block_reason = option_get_occupied(original_step_blocked_reasons).unwrap();
                match block_reason.get_value() {
                    OriginalStepBlockedReason::SourceState {
                        source_channel,
                    } => {
                        let source_channel =
                            btree_map_get_occupied(blocked_source_channels, *source_channel)
                                .unwrap();
                        CallerChannelStatus::OriginalStepSourceState(OriginalStepSourceState {
                            caller_channel,
                            step,
                            block_reason,
                            source_channel,
                            repeat_step_blocked_reasons,
                        })
                    },
                    OriginalStepBlockedReason::Future => {
                        CallerChannelStatus::OriginalStepFuture(OriginalStepFuture {
                            caller_channel,
                            step,
                            block_reason,
                            blocked_source_channels,
                            repeat_step_blocked_reasons,
                        })
                    },
                }
            },
            CallerChannelBlockedReason::RepeatStep {
                step_id,
            } => {
                let step = steps.get_entry_by_sequence_number(*step_id).unwrap();
                let block_reason =
                    hash_map_get_occupied(repeat_step_blocked_reasons, *step_id).unwrap();
                match block_reason.get_value() {
                    RepeatStepBlockedReason::SourceState {
                        source_channel, ..
                    } => {
                        let source_channel =
                            btree_map_get_occupied(blocked_source_channels, *source_channel)
                                .unwrap();
                        CallerChannelStatus::RepeatStepSourceState(RepeatStepSourceState {
                            caller_channel,
                            step,
                            block_reason,
                            source_channel,
                            original_step_blocked_reasons,
                        })
                    },
                    RepeatStepBlockedReason::Future {
                        ..
                    } => CallerChannelStatus::RepeatStepFuture(RepeatStepFuture {
                        caller_channel,
                        step,
                        block_reason,
                        blocked_source_channels,
                        original_step_blocked_reasons,
                    }),
                }
            },
            CallerChannelBlockedReason::InterpolationSourceState {
                source_channel, ..
            } => {
                let source_channel =
                    btree_map_get_occupied(blocked_source_channels, *source_channel).unwrap();
                CallerChannelStatus::InterpolationSourceState(InterpolationSourceState {
                    caller_channel,
                    source_channel,
                    steps,
                    repeat_step_blocked_reasons,
                    original_step_blocked_reasons,
                })
            },
            CallerChannelBlockedReason::InterpolationFuture {
                ..
            } => CallerChannelStatus::InterpolationFuture(InterpolationFuture {
                caller_channel,
                steps,
                blocked_source_channels,
                repeat_step_blocked_reasons,
                original_step_blocked_reasons,
            }),
            Poisioned => panic!(),
        }
    }
}

#[derive(Debug)]
pub enum RepeatStepBlockedReason {
    SourceState {
        source_channel: usize,
        stack_waker:    Weak<StackWaker>,
    },
    Future {
        stack_waker: Weak<StackWaker>,
    },
}

#[derive(Debug)]
pub enum OriginalStepBlockedReason {
    SourceState { source_channel: usize },
    Future,
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
    Poisioned,
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

/// this enum represents the current blocked status for a given channel.
/// it can move between statuses under various circumstances,
/// like being provided a source state, or a future polling ready.
pub enum CallerChannelStatus<'a, T: Transposer> {
    Free(Free<'a, T>),
    OriginalStepSourceState(OriginalStepSourceState<'a, T>),
    OriginalStepFuture(OriginalStepFuture<'a, T>),
    RepeatStepSourceState(RepeatStepSourceState<'a, T>),
    RepeatStepFuture(RepeatStepFuture<'a, T>),
    InterpolationSourceState(InterpolationSourceState<'a, T>),
    InterpolationFuture(InterpolationFuture<'a, T>),
    Limbo,
}

pub struct Free<'a, T: Transposer> {
    // entries
    caller_channel: HashMapVacantEntry<'a, usize, CallerChannelBlockedReason<T>>,

    // extra
    steps: &'a mut Steps<T>,
    blocked_source_channels: &'a mut BTreeMap<usize, ()>,
    repeat_step_blocked_reasons: &'a mut HashMap<usize, RepeatStepBlockedReason>,
    original_step_blocked_reasons: &'a mut Option<OriginalStepBlockedReason>,
}

pub struct OriginalStepSourceState<'a, T: Transposer> {
    // entries
    caller_channel: HashMapOccupiedEntry<'a, usize, CallerChannelBlockedReason<T>>,
    step:           VecDequeEntry<'a, StepWrapper<T>>,
    block_reason:   OptionOccupiedEntry<'a, OriginalStepBlockedReason>,
    source_channel: BTreeMapOccupiedEntry<'a, usize, ()>,

    // extra
    repeat_step_blocked_reasons: &'a mut HashMap<usize, RepeatStepBlockedReason>,
}

pub struct OriginalStepFuture<'a, T: Transposer> {
    // entries
    caller_channel: HashMapOccupiedEntry<'a, usize, CallerChannelBlockedReason<T>>,
    step:           VecDequeEntry<'a, StepWrapper<T>>,
    block_reason:   OptionOccupiedEntry<'a, OriginalStepBlockedReason>,

    // extra
    blocked_source_channels:     &'a mut BTreeMap<usize, ()>,
    repeat_step_blocked_reasons: &'a mut HashMap<usize, RepeatStepBlockedReason>,
}

pub struct RepeatStepSourceState<'a, T: Transposer> {
    // entries
    caller_channel: HashMapOccupiedEntry<'a, usize, CallerChannelBlockedReason<T>>,
    step:           VecDequeEntry<'a, StepWrapper<T>>,
    block_reason:   HashMapOccupiedEntry<'a, usize, RepeatStepBlockedReason>,
    source_channel: BTreeMapOccupiedEntry<'a, usize, ()>,

    // extra
    original_step_blocked_reasons: &'a mut Option<OriginalStepBlockedReason>,
}

pub struct RepeatStepFuture<'a, T: Transposer> {
    // entries
    caller_channel: HashMapOccupiedEntry<'a, usize, CallerChannelBlockedReason<T>>,
    step:           VecDequeEntry<'a, StepWrapper<T>>,
    block_reason:   HashMapOccupiedEntry<'a, usize, RepeatStepBlockedReason>,

    // extra
    blocked_source_channels:       &'a mut BTreeMap<usize, ()>,
    original_step_blocked_reasons: &'a mut Option<OriginalStepBlockedReason>,
}

pub struct InterpolationSourceState<'a, T: Transposer> {
    // entries
    caller_channel: HashMapOccupiedEntry<'a, usize, CallerChannelBlockedReason<T>>,
    source_channel: BTreeMapOccupiedEntry<'a, usize, ()>,

    // extra
    steps: &'a mut Steps<T>,
    repeat_step_blocked_reasons: &'a mut HashMap<usize, RepeatStepBlockedReason>,
    original_step_blocked_reasons: &'a mut Option<OriginalStepBlockedReason>,
}

pub struct InterpolationFuture<'a, T: Transposer> {
    // entries
    caller_channel: HashMapOccupiedEntry<'a, usize, CallerChannelBlockedReason<T>>,

    // extra
    steps: &'a mut Steps<T>,
    blocked_source_channels: &'a mut BTreeMap<usize, ()>,
    repeat_step_blocked_reasons: &'a mut HashMap<usize, RepeatStepBlockedReason>,
    original_step_blocked_reasons: &'a mut Option<OriginalStepBlockedReason>,
}

impl<'a, T: Transposer> Free<'a, T> {
    /// either OriginalStepFuture, RepeatStepFuture, or InterpolationFuture
    pub fn poll(self, forget: bool, time: T::Time) -> CallerChannelStatus<'a, T> {
        let Self {
            caller_channel,
            steps,
            blocked_source_channels,
            repeat_step_blocked_reasons,
            original_step_blocked_reasons,
        } = self;
        match steps.get_before_or_at(time).unwrap() {
            super::steps::BeforeStatus::SaturatedImmediate(step) => {
                let interpolation = step.step.interpolate(time).unwrap();
                let blocked_reason = CallerChannelBlockedReason::InterpolationFuture {
                    interpolation,
                    forget,
                };
                let caller_channel = caller_channel.occupy(blocked_reason);

                CallerChannelStatus::InterpolationFuture(InterpolationFuture {
                    caller_channel,
                    steps,
                    blocked_source_channels,
                    repeat_step_blocked_reasons,
                    original_step_blocked_reasons,
                })
            },
            super::steps::BeforeStatus::SaturatedDistant(saturated, next) => {
                // start saturating next

                todo!()
            },
            super::steps::BeforeStatus::Saturating(step) => {
                // join the saturating.

                todo!()
            },
        }
    }

    /// if there's nothing to do, return the time for scheduled.
    pub fn poll_events(self, time: T::Time) -> Result<Option<T::Time>, CallerChannelStatus<'a, T>> {
        todo!()
    }
}

impl<'a, T: Transposer> OriginalStepFuture<'a, T> {
    pub fn poll(self, all_channel_waker: &Waker) -> (CallerChannelStatus<'a, T>, Vec<T::Output>) {
        todo!()
    }

    pub fn time(&self) -> T::Time {
        todo!()
    }
}

impl<'a, T: Transposer> OriginalStepSourceState<'a, T> {
    pub fn get_args_for_source_poll(&mut self) -> (T::Time, /* source channel */ usize) {
        match self.block_reason.get_value_mut() {
            OriginalStepBlockedReason::SourceState {
                source_channel,
            } => {
                let time = self.step.get_value().step.raw_time();
                (time, *source_channel)
            },
            _ => unreachable!(),
        }
    }

    pub fn provide_state(self, state: T::InputState, skip_wake: bool) -> OriginalStepFuture<'a, T> {
        let OriginalStepSourceState {
            // entries
            caller_channel,
            mut step,
            block_reason,
            source_channel,

            // extra
            repeat_step_blocked_reasons,
        } = self;

        let x = step.get_value_mut().step.set_input_state(state, skip_wake);

        // TODO figure out error handling strategy.
        debug_assert!(x.is_ok());

        // we're not blocked anymore, so we can remove the blocked source channel.
        let (vacant, ()) = source_channel.vacate();
        let (blocked_source_channels, _) = vacant.into_collection_mut();

        OriginalStepFuture {
            // entries
            caller_channel,
            step,
            block_reason,

            // extra
            blocked_source_channels,
            repeat_step_blocked_reasons,
        }
    }
}

impl<'a, T: Transposer> RepeatStepFuture<'a, T> {
    pub fn poll(mut self, one_channel_waker: &Waker) -> Poll<CallerChannelStatus<'a, T>> {
        let stack_waker = match self.block_reason.get_value_mut() {
            RepeatStepBlockedReason::Future {
                stack_waker,
            } => stack_waker,
            _ => unreachable!(),
        };

        let channel = *self.caller_channel.get_key();

        let waker = match StackWaker::register(stack_waker, channel, one_channel_waker.clone()) {
            Some(w) => w,
            None => return Poll::Pending,
        };

        let StepPoll {
            result,
            outputs,
        } = self.step.get_value_mut().step.poll(waker).unwrap();

        #[cfg(debug_assertions)]
        assert!(outputs.is_empty());

        let Self {
            caller_channel,
            step,
            block_reason,
            blocked_source_channels,
            original_step_blocked_reasons,
        } = self;

        let next_status = match result {
            transposer::step::StepPollResult::NeedsState => RepeatStepSourceState {
                caller_channel,
                step,
                block_reason,
                source_channel: get_first_vacant(blocked_source_channels).occupy(()),
                original_step_blocked_reasons,
            },
            transposer::step::StepPollResult::Pending => return Poll::Pending,
            transposer::step::StepPollResult::Ready => {
                let (caller_channel, _) = caller_channel.vacate();

                return todo!()
                // Free {
                //     caller_channel,
                //     steps: todo!(),
                //     blocked_source_channels,
                //     repeat_step_blocked_reasons: todo!(),
                //     original_step_blocked_reasons,
                // }
            },
        };

        // Poll::Ready(next_status)
        todo!()
    }
}

impl<'a, T: Transposer> RepeatStepSourceState<'a, T> {
    pub fn get_args_for_source_poll(
        &mut self,
    ) -> (
        T::Time,
        &mut Weak<StackWaker>,
        /* source channel */ usize,
    ) {
        match self.block_reason.get_value_mut() {
            RepeatStepBlockedReason::SourceState {
                source_channel,
                stack_waker,
            } => {
                let time = self.step.get_value().step.raw_time();
                (time, stack_waker, *source_channel)
            },
            _ => unreachable!(),
        }
    }
    pub fn provide_state(self, state: T::InputState, skip_wake: bool) -> RepeatStepFuture<'a, T> {
        let RepeatStepSourceState {
            // entries
            caller_channel,
            mut step,
            block_reason,
            source_channel,
            // extra
            original_step_blocked_reasons,
        } = self;

        let x = step.get_value_mut().step.set_input_state(state, skip_wake);

        // TODO figure out error handling strategy.
        debug_assert!(x.is_ok());

        // we're not blocked anymore, so we can remove the blocked source channel.
        let (vacant, ()) = source_channel.vacate();
        let (blocked_source_channels, _) = vacant.into_collection_mut();

        RepeatStepFuture {
            // entries
            caller_channel,
            step,
            block_reason,

            // extra
            blocked_source_channels,
            original_step_blocked_reasons,
        }
    }
}

impl<'a, T: Transposer> InterpolationFuture<'a, T> {
    pub fn poll(
        mut self,
        one_channel_waker: &Waker,
    ) -> Result<Poll<T::OutputState>, CallerChannelStatus<'a, T>> {
        let interpolation = match self.caller_channel.get_value_mut() {
            CallerChannelBlockedReason::InterpolationFuture {
                interpolation,
                forget: _,
            } => interpolation,
            _ => unreachable!("CallerChannelBlockedReason does not match CallerChannelStatus"),
        };
        let mut cx = Context::from_waker(one_channel_waker);
        let mut interpolation = Pin::new(interpolation);
        if let Poll::Ready(o) = interpolation.as_mut().poll(&mut cx) {
            return Ok(Poll::Ready(o))
        }

        if !interpolation.needs_state() {
            return Ok(Poll::Pending)
        }

        let Self {
            mut caller_channel,
            steps,
            blocked_source_channels,
            repeat_step_blocked_reasons,
            original_step_blocked_reasons,
        } = self;

        let vacant = get_first_vacant(blocked_source_channels);
        let source_channel = vacant.occupy(());

        replace(
            caller_channel.get_value_mut(),
            || CallerChannelBlockedReason::Poisioned,
            |c| match c {
                CallerChannelBlockedReason::InterpolationFuture {
                    interpolation,
                    forget,
                } => CallerChannelBlockedReason::InterpolationSourceState {
                    source_channel: *source_channel.get_key(),
                    interpolation,
                    forget,
                },
                _ => unreachable!(),
            },
        );

        let inner = InterpolationSourceState {
            caller_channel,
            source_channel,
            steps,
            repeat_step_blocked_reasons,
            original_step_blocked_reasons,
        };

        Err(CallerChannelStatus::InterpolationSourceState(inner))
    }
}

impl<'a, T: Transposer> InterpolationSourceState<'a, T> {
    // don't need time or waker because they're just passed through for interpolations.
    pub fn get_args_for_source_poll(&self) -> (/* source channel */ usize, /* forget */ bool) {
        match self.caller_channel.get_value() {
            CallerChannelBlockedReason::InterpolationSourceState {
                source_channel,
                interpolation: _,
                forget,
            } => (*source_channel, *forget),
            _ => unreachable!("CallerChannelBlockedReason does not match CallerChannelStatus"),
        }
    }
    pub fn provide_state(
        self,
        state: T::InputState,
        skip_wake: bool,
    ) -> InterpolationFuture<'a, T> {
        let InterpolationSourceState {
            // entries
            mut caller_channel,
            source_channel,

            // extra
            steps,
            repeat_step_blocked_reasons,
            original_step_blocked_reasons,
        } = self;

        if let CallerChannelBlockedReason::InterpolationSourceState {
            interpolation, ..
        } = caller_channel.get_value_mut()
        {
            let x = interpolation.set_state(state, skip_wake);

            // TODO figure out error handling strategy.
            debug_assert!(x.is_ok())
        } else {
            panic!()
        }

        // we're not blocked anymore, so we can remove the blocked source channel.
        let (vacant, ()) = source_channel.vacate();
        let (blocked_source_channels, _) = vacant.into_collection_mut();

        InterpolationFuture {
            // entries
            caller_channel,

            // extra
            steps,
            blocked_source_channels,
            repeat_step_blocked_reasons,
            original_step_blocked_reasons,
        }
    }
}
