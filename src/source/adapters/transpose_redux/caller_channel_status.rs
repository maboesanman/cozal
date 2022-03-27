use std::collections::{BTreeMap, HashMap};
use std::task::Poll;

use super::channel_assignments::{CallerChannelBlockedReason, StepBlockedReason, TransposerInner};
use super::steps::{StepWrapper, Steps};
use super::Transpose;
use crate::source::traits::SourceContext;
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
    fn get_channel_status(inner: &'a mut TransposerInner<T>, caller_channel: usize) -> Self {
        let TransposerInner {
            ref mut blocked_caller_channels,
            ref mut steps,
            ref mut blocked_source_channels,
            ref mut repeat_step_blocked_reasons,
            ref mut original_step_blocked_reasons,
        } = inner;

        let caller_channel_entry = hash_map_get_occupied(blocked_caller_channels, caller_channel);

        let caller_channel = match caller_channel_entry {
            Err(caller_channel) => {
                return Self::Free(Free {
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
                        Self::OriginalStepSourceState(OriginalStepSourceState {
                            caller_channel,
                            step,
                            block_reason,
                            source_channel,
                            repeat_step_blocked_reasons,
                        })
                    },
                    StepBlockedReason::Future {
                        ..
                    } => Self::OriginalStepFuture(OriginalStepFuture {
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
                        Self::RepeatStepSourceState(RepeatStepSourceState {
                            caller_channel,
                            step,
                            block_reason,
                            source_channel,
                            original_step_blocked_reasons,
                        })
                    },
                    StepBlockedReason::Future {
                        ..
                    } => Self::RepeatStepFuture(RepeatStepFuture {
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
                Self::InterpolationSourceState(InterpolationSourceState {
                    caller_channel,
                    source_channel,
                    steps,
                    repeat_step_blocked_reasons,
                    original_step_blocked_reasons,
                })
            },
            CallerChannelBlockedReason::InterpolationFuture {
                ..
            } => Self::InterpolationFuture(InterpolationFuture {
                caller_channel,
                steps,
                blocked_source_channels,
                repeat_step_blocked_reasons,
                original_step_blocked_reasons,
            }),
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
    pub fn poll(self) -> CallerChannelStatus<'a, T> {
        todo!()
    }
}

impl<'a, T: Transposer> OriginalStepFuture<'a, T> {
    pub fn poll(self) -> CallerChannelStatus<'a, T> {
        todo!()
    }
}

impl<'a, T: Transposer> OriginalStepSourceState<'a, T> {
    pub fn get_context_for_source_poll(&self) -> SourceContext {
        todo!()
    }
    pub fn provide_state(self) -> CallerChannelStatus<'a, T> {
        todo!()
    }
}

impl<'a, T: Transposer> RepeatStepFuture<'a, T> {
    pub fn poll(self) -> CallerChannelStatus<'a, T> {
        todo!()
    }
}

impl<'a, T: Transposer> RepeatStepSourceState<'a, T> {
    pub fn get_context_for_source_poll(&self) -> SourceContext {
        todo!()
    }
    pub fn provide_state(self) -> CallerChannelStatus<'a, T> {
        todo!()
    }
}

impl<'a, T: Transposer> InterpolationFuture<'a, T> {
    pub fn poll(self) -> CallerChannelStatus<'a, T> {
        todo!()
    }
}

impl<'a, T: Transposer> InterpolationSourceState<'a, T> {
    pub fn get_context_for_source_poll(&self) -> SourceContext {
        todo!()
    }
    pub fn provide_state(self) -> CallerChannelStatus<'a, T> {
        todo!()
    }
}
