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

use super::original_step_future::OriginalStepFuture;
use super::{CallerChannelBlockedReason, StepBlockedReason, RepeatStepBlockedReason};


pub struct OriginalStepSourceState<'a, T: Transposer> {
    // entries
    pub caller_channel: HashMapOccupiedEntry<'a, usize, CallerChannelBlockedReason<T>>,
    pub block_reason:   OptionOccupiedEntry<'a, StepBlockedReason>,
    pub source_channel: BTreeMapOccupiedEntry<'a, usize, ()>,

    // extra
    pub blocked_repeat_steps: &'a mut HashMap<usize, RepeatStepBlockedReason>,
}

impl<'a, T: Transposer> OriginalStepSourceState<'a, T> {
    pub fn get_args_for_source_poll(&mut self) -> (T::Time, /* source channel */ usize) {
        // match self.block_reason.get_value_mut() {
        //     StepBlockedReason::SourceState {
        //         source_channel,
        //     } => {
        //         let time = self.step.get_value().step.raw_time();
        //         (time, *source_channel)
        //     },
        //     _ => unreachable!(),
        // }
        todo!()
    }

    pub fn provide_state(self, state: T::InputState) -> OriginalStepFuture<'a, T> {
        todo!()
        // let OriginalStepSourceState {
        //     // entries
        //     caller_channel,
        //     mut step,
        //     block_reason,
        //     source_channel,

        //     // extra
        //     repeat_step_blocked_reasons,
        // } = self;

        // let x = step.get_value_mut().step.set_input_state(state);

        // // TODO figure out error handling strategy.
        // debug_assert!(x.is_ok());

        // // we're not blocked anymore, so we can remove the blocked source channel.
        // let (vacant, ()) = source_channel.vacate();
        // let (blocked_source_channels, _) = vacant.into_collection_mut();

        // OriginalStepFuture {
        //     // entries
        //     caller_channel,
        //     step,
        //     block_reason,

        //     // extra
        //     blocked_source_channels,
        //     repeat_step_blocked_reasons,
        // }
    }
}