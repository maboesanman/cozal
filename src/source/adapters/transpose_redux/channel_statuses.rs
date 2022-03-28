use std::collections::{BTreeMap, HashMap};
use std::sync::Weak;
use std::task::Waker;

use super::steps::{StepWrapper, Steps};
use super::storage::TransposeStorage;
// use crate::source::adapters::transpose_redux::transpose_step_metadata::unwrap_repeat_saturating;
use crate::source::traits::SourceContext;
use crate::transposer::step::Interpolation;
use crate::transposer::Transposer;
use crate::util::extended_entry::btree_map::{
    get_occupied as btree_map_get_occupied,
    OccupiedExtEntry as BTreeMapOccupiedEntry,
    VacantExtEntry as BTreeMapVacantEntry,
};
use crate::util::extended_entry::hash_map::{
    get_occupied as hash_map_get_occupied,
    OccupiedExtEntry as HashMapOccupiedEntry,
    VacantExtEntry as HashMapVacantEntry,
};
use crate::util::extended_entry::option::{
    get_occupied as option_get_occupied,
    OccupiedExtEntry as OptionOccupiedEntry,
    VacantExtEntry as OptionVacantEntry,
};
use crate::util::extended_entry::vecdeque::{
    get_ext_entry as vecdeque_get_ext_entry,
    ExtEntry as VecDequeEntry,
};
use crate::util::replace_waker::ReplaceWaker;
use crate::util::stack_waker::StackWaker;

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
    pub repeat_step_blocked_reasons: HashMap</* step_id */ usize, StepBlockedReason>,

    // this is the currently blocked original step if it exists, including the stack wakers
    pub original_step_blocked_reasons: Option<StepBlockedReason>,
}

impl<T: Transposer> ChannelStatuses<T> {
    pub fn new(transposer: T, rng_seed: [u8; 32]) -> Self {
        todo!()
    }

    pub fn get_channel_status(&mut self, caller_channel: usize) -> CallerChannelStatus<'_, T> {
        let ChannelStatuses {
            ref mut blocked_caller_channels,
            ref mut steps,
            ref mut blocked_source_channels,
            ref mut repeat_step_blocked_reasons,
            ref mut original_step_blocked_reasons,
        } = self;

        let caller_channel_entry = hash_map_get_occupied(blocked_caller_channels, caller_channel);

        let caller_channel = match caller_channel_entry {
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

        match caller_channel.get_value() {
            CallerChannelBlockedReason::OriginalStep => {
                let step = steps.get_last_entry().unwrap();
                let block_reason = option_get_occupied(original_step_blocked_reasons).unwrap();
                match block_reason.get_value() {
                    StepBlockedReason::SourceState {
                        source_channel, ..
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
                    StepBlockedReason::Future {
                        ..
                    } => CallerChannelStatus::OriginalStepFuture(OriginalStepFuture {
                        caller_channel,
                        step,
                        block_reason,
                        blocked_source_channels,
                        repeat_step_blocked_reasons,
                    }),
                }
            },
            CallerChannelBlockedReason::RepeatStep {
                step_id,
            } => {
                let step = steps.get_entry_by_sequence_number(*step_id).unwrap();
                let block_reason =
                    hash_map_get_occupied(repeat_step_blocked_reasons, *step_id).unwrap();
                match block_reason.get_value() {
                    StepBlockedReason::SourceState {
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
                    StepBlockedReason::Future {
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
        }
    }
}

#[derive(Debug)]
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
}

impl<'a, T: Transposer> CallerChannelStatus<'a, T> {
    pub fn caller_channel(&self) -> usize {
        match self {
            CallerChannelStatus::Free(s) => s.caller_channel(),
            CallerChannelStatus::OriginalStepSourceState(s) => s.caller_channel(),
            CallerChannelStatus::OriginalStepFuture(s) => s.caller_channel(),
            CallerChannelStatus::RepeatStepSourceState(s) => s.caller_channel(),
            CallerChannelStatus::RepeatStepFuture(s) => s.caller_channel(),
            CallerChannelStatus::InterpolationSourceState(s) => s.caller_channel(),
            CallerChannelStatus::InterpolationFuture(s) => s.caller_channel(),
        }
    }
}

pub struct Free<'a, T: Transposer> {
    // entries
    caller_channel: HashMapVacantEntry<'a, usize, CallerChannelBlockedReason<T>>,

    // extra
    steps: &'a mut Steps<T>,
    blocked_source_channels: &'a mut BTreeMap<usize, ()>,
    repeat_step_blocked_reasons: &'a mut HashMap<usize, StepBlockedReason>,
    original_step_blocked_reasons: &'a mut Option<StepBlockedReason>,
}

pub struct OriginalStepSourceState<'a, T: Transposer> {
    // entries
    caller_channel: HashMapOccupiedEntry<'a, usize, CallerChannelBlockedReason<T>>,
    step:           VecDequeEntry<'a, StepWrapper<T>>,
    block_reason:   OptionOccupiedEntry<'a, StepBlockedReason>,
    source_channel: BTreeMapOccupiedEntry<'a, usize, ()>,

    // extra
    repeat_step_blocked_reasons: &'a mut HashMap<usize, StepBlockedReason>,
}

pub struct OriginalStepFuture<'a, T: Transposer> {
    // entries
    caller_channel: HashMapOccupiedEntry<'a, usize, CallerChannelBlockedReason<T>>,
    step:           VecDequeEntry<'a, StepWrapper<T>>,
    block_reason:   OptionOccupiedEntry<'a, StepBlockedReason>,

    // extra
    blocked_source_channels:     &'a mut BTreeMap<usize, ()>,
    repeat_step_blocked_reasons: &'a mut HashMap<usize, StepBlockedReason>,
}

pub struct RepeatStepSourceState<'a, T: Transposer> {
    // entries
    caller_channel: HashMapOccupiedEntry<'a, usize, CallerChannelBlockedReason<T>>,
    step:           VecDequeEntry<'a, StepWrapper<T>>,
    block_reason:   HashMapOccupiedEntry<'a, usize, StepBlockedReason>,
    source_channel: BTreeMapOccupiedEntry<'a, usize, ()>,

    // extra
    original_step_blocked_reasons: &'a mut Option<StepBlockedReason>,
}

pub struct RepeatStepFuture<'a, T: Transposer> {
    // entries
    caller_channel: HashMapOccupiedEntry<'a, usize, CallerChannelBlockedReason<T>>,
    step:           VecDequeEntry<'a, StepWrapper<T>>,
    block_reason:   HashMapOccupiedEntry<'a, usize, StepBlockedReason>,

    // extra
    blocked_source_channels:       &'a mut BTreeMap<usize, ()>,
    original_step_blocked_reasons: &'a mut Option<StepBlockedReason>,
}

pub struct InterpolationSourceState<'a, T: Transposer> {
    // entries
    caller_channel: HashMapOccupiedEntry<'a, usize, CallerChannelBlockedReason<T>>,
    source_channel: BTreeMapOccupiedEntry<'a, usize, ()>,

    // extra
    steps: &'a mut Steps<T>,
    repeat_step_blocked_reasons: &'a mut HashMap<usize, StepBlockedReason>,
    original_step_blocked_reasons: &'a mut Option<StepBlockedReason>,
}

pub struct InterpolationFuture<'a, T: Transposer> {
    // entries
    caller_channel: HashMapOccupiedEntry<'a, usize, CallerChannelBlockedReason<T>>,

    // extra
    steps: &'a mut Steps<T>,
    blocked_source_channels: &'a mut BTreeMap<usize, ()>,
    repeat_step_blocked_reasons: &'a mut HashMap<usize, StepBlockedReason>,
    original_step_blocked_reasons: &'a mut Option<StepBlockedReason>,
}

impl<'a, T: Transposer> Free<'a, T> {
    pub fn caller_channel(&self) -> usize {
        *self.caller_channel.get_key()
    }
    pub fn poll(self) -> Result<T::OutputState, CallerChannelStatus<'a, T>> {
        todo!()
    }
}

impl<'a, T: Transposer> OriginalStepFuture<'a, T> {
    pub fn caller_channel(&self) -> usize {
        *self.caller_channel.get_key()
    }
    pub fn get_waker_for_future_poll(&self) -> Option<Waker> {
        todo!()
    }
    pub fn poll_step(self) -> (CallerChannelStatus<'a, T>, Vec<T::Output>) {
        todo!()
    }
}

impl<'a, T: Transposer> OriginalStepSourceState<'a, T> {
    pub fn caller_channel(&self) -> usize {
        *self.caller_channel.get_key()
    }
    pub fn get_context_for_source_poll(&self) -> Option<SourceContext> {
        todo!()
    }
    pub fn provide_state(self) -> CallerChannelStatus<'a, T> {
        todo!()
    }
}

impl<'a, T: Transposer> RepeatStepFuture<'a, T> {
    pub fn caller_channel(&self) -> usize {
        *self.caller_channel.get_key()
    }
    pub fn get_waker_for_future_poll(&self) -> Option<Waker> {
        todo!()
    }
    pub fn poll_step(self) -> CallerChannelStatus<'a, T> {
        todo!()
    }
}

impl<'a, T: Transposer> RepeatStepSourceState<'a, T> {
    pub fn caller_channel(&self) -> usize {
        *self.caller_channel.get_key()
    }
    pub fn get_context_for_source_poll(&self) -> Option<SourceContext> {
        todo!()
    }
    pub fn provide_state(self) -> CallerChannelStatus<'a, T> {
        todo!()
    }
}

impl<'a, T: Transposer> InterpolationFuture<'a, T> {
    pub fn caller_channel(&self) -> usize {
        *self.caller_channel.get_key()
    }
    pub fn poll_interpolation(self) -> CallerChannelStatus<'a, T> {
        todo!()
    }
}

impl<'a, T: Transposer> InterpolationSourceState<'a, T> {
    pub fn caller_channel(&self) -> usize {
        *self.caller_channel.get_key()
    }
    pub fn get_context_for_source_poll(&self) -> SourceContext {
        todo!()
    }
    pub fn provide_state(self) -> CallerChannelStatus<'a, T> {
        todo!()
    }
}
