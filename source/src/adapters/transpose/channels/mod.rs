use std::collections::{BTreeMap, HashMap};
use std::pin::Pin;
use std::sync::Weak;
use std::task::{Context, Poll, Waker};

use futures_core::Future;
use transposer::step::{Interpolation, NextInputs, StepPoll};
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
use util::extended_entry::vecdeque::get_ext_entry as vecdeque_get_ext_entry;
use util::replace_mut::replace;
use util::stack_waker::StackWaker;

use self::interpolation_future::InterpolationFuture;
use self::interpolation_source_state::InterpolationSourceState;
use self::free::Free;
use self::original_step_future::OriginalStepFuture;
use self::original_step_source_state::OriginalStepSourceState;
use self::repeat_step_future::RepeatStepFuture;
use self::repeat_step_source_state::RepeatStepSourceState;

use super::steps::{StepWrapper, Steps};
use super::storage::TransposeStorage;

mod free;
mod interpolation_future;
mod interpolation_source_state;
mod original_step_future;
mod original_step_source_state;
mod repeat_step_future;
mod repeat_step_source_state;

// manage the association of source and caller channels.
// own steps.
// own interpolations.
// own one_channel_wakers.
pub struct ChannelStatuses<T: Transposer> {
    // These are all the source channels currently in use.
    // this would be a BTreeSet but that doesn't have the entry api...
    pub blocked_source_channels: BTreeMap</* source_channel */ usize, ()>,

    // These are all the currently pending operations, from the perspective of the caller.
    // They can be blocked due to pending source_state, step_future, or interpolation_future.
    pub blocked_caller_channels:
        HashMap</* caller_channel */ usize, CallerChannelBlockedReason<T>>,

    // these are the currently blocked repeat steps, including the stack wakers
    pub blocked_repeat_steps: HashMap</* step_id */ usize, RepeatStepBlockedReason>,

    // this is the currently blocked original step if it is blocked, including the stack wakers
    pub blocked_original_step: Option<StepBlockedReason>,
}

impl<T: Transposer> ChannelStatuses<T> {
    pub fn new() -> Self {
        Self {
            blocked_source_channels: BTreeMap::new(),
            blocked_caller_channels: HashMap::new(),
            blocked_repeat_steps: HashMap::new(),
            blocked_original_step: None,
        }
    }

    /// from the current state, get the status.
    ///
    /// this internally holds mutable refs to the ChannelStatuses
    pub fn get_channel_status(&mut self, caller_channel: usize) -> CallerChannelStatus<'_, T> {
        // let ChannelStatuses {
        //     ref mut blocked_caller_channels,
        //     ref mut steps,
        //     ref mut blocked_source_channels,
        //     blocked_repeat_steps: ref mut repeat_step_blocked_reasons,
        //     blocked_original_step: ref mut original_step_blocked_reasons,
        // } = self;

        // let caller_channel_entry = hash_map_get_occupied(blocked_caller_channels, caller_channel);

        // // get current blocker, or free if it isn't blocked.
        // let mut caller_channel = match caller_channel_entry {
        //     Err(caller_channel) => {
        //         return CallerChannelStatus::Free(Free {
        //             caller_channel,
        //             steps,
        //             blocked_source_channels,
        //             repeat_step_blocked_reasons,
        //             original_step_blocked_reasons,
        //         })
        //     },
        //     Ok(caller_channel) => caller_channel,
        // };

        // match caller_channel.get_value_mut() {
        //     CallerChannelBlockedReason::OriginalStep => {
        //         let step = steps.get_last_entry().unwrap();
        //         let block_reason = option_get_occupied(original_step_blocked_reasons).unwrap();
        //         match block_reason.get_value() {
        //             OriginalStepBlockedReason::SourceState {
        //                 source_channel,
        //             } => {
        //                 let source_channel =
        //                     btree_map_get_occupied(blocked_source_channels, *source_channel)
        //                         .unwrap();
        //                 CallerChannelStatus::OriginalStepSourceState(OriginalStepSourceState {
        //                     caller_channel,
        //                     step,
        //                     block_reason,
        //                     source_channel,
        //                     repeat_step_blocked_reasons,
        //                 })
        //             },
        //             OriginalStepBlockedReason::Future => {
        //                 CallerChannelStatus::OriginalStepFuture(OriginalStepFuture {
        //                     caller_channel,
        //                     step,
        //                     block_reason,
        //                     blocked_source_channels,
        //                     repeat_step_blocked_reasons,
        //                 })
        //             },
        //         }
        //     },
        //     CallerChannelBlockedReason::RepeatStep {
        //         step_id,
        //     } => {
        //         let step = steps.get_entry_by_sequence_number(*step_id).unwrap();
        //         let block_reason =
        //             hash_map_get_occupied(repeat_step_blocked_reasons, *step_id).unwrap();
        //         match block_reason.get_value() {
        //             RepeatStepBlockedReason::SourceState {
        //                 source_channel, ..
        //             } => {
        //                 let source_channel =
        //                     btree_map_get_occupied(blocked_source_channels, *source_channel)
        //                         .unwrap();
        //                 CallerChannelStatus::RepeatStepSourceState(RepeatStepSourceState {
        //                     caller_channel,
        //                     step,
        //                     block_reason,
        //                     source_channel,
        //                     original_step_blocked_reasons,
        //                 })
        //             },
        //             RepeatStepBlockedReason::Future {
        //                 ..
        //             } => CallerChannelStatus::RepeatStepFuture(RepeatStepFuture {
        //                 caller_channel,
        //                 step,
        //                 block_reason,
        //                 blocked_source_channels,
        //                 original_step_blocked_reasons,
        //             }),
        //         }
        //     },
        //     CallerChannelBlockedReason::InterpolationSourceState {
        //         source_channel, ..
        //     } => {
        //         let source_channel =
        //             btree_map_get_occupied(blocked_source_channels, *source_channel).unwrap();
        //         CallerChannelStatus::InterpolationSourceState(InterpolationSourceState {
        //             caller_channel,
        //             source_channel,
        //             steps,
        //             repeat_step_blocked_reasons,
        //             original_step_blocked_reasons,
        //         })
        //     },
        //     CallerChannelBlockedReason::InterpolationFuture {
        //         ..
        //     } => CallerChannelStatus::InterpolationFuture(InterpolationFuture {
        //         caller_channel,
        //         steps,
        //         blocked_source_channels,
        //         repeat_step_blocked_reasons,
        //         original_step_blocked_reasons,
        //     }),
        //     Poisioned => panic!(),
        // }
        todo!()
    }
}


#[derive(Debug)]
pub enum StepBlockedReason {
    SourceState { source_channel: usize },
    Future,
}

#[derive(Debug)]
pub struct RepeatStepBlockedReason {
    reason: StepBlockedReason,
    waker: Weak<StackWaker>,
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
    InterpolationFuture(InterpolationFuture<'a, T>),
    InterpolationSourceState(InterpolationSourceState<'a, T>),
    OriginalStepFuture(OriginalStepFuture<'a, T>),
    OriginalStepSourceState(OriginalStepSourceState<'a, T>),
    RepeatStepFuture(RepeatStepFuture<'a, T>),
    RepeatStepSourceState(RepeatStepSourceState<'a, T>),
    Limbo,
}