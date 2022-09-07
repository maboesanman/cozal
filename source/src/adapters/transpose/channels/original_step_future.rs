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

use super::{CallerChannelBlockedReason, StepBlockedReason, CallerChannelStatus, RepeatStepBlockedReason};


pub struct OriginalStepFuture<'a, T: Transposer> {
    // entries
    pub caller_channel: HashMapOccupiedEntry<'a, usize, CallerChannelBlockedReason<T>>,
    pub block_reason:   OptionOccupiedEntry<'a, StepBlockedReason>,

    // extra
    pub blocked_source_channels: &'a mut BTreeMap<usize, ()>,
    pub blocked_repeat_steps: &'a mut HashMap<usize, RepeatStepBlockedReason>,
}

impl<'a, T: Transposer> OriginalStepFuture<'a, T> {
    pub fn poll(
        mut self,
        all_channel_waker: &Waker,
        next_inputs: &mut NextInputs<T>,
    ) -> (CallerChannelStatus<'a, T>, Option<T::Output>) {
        // match self
        //     .step
        //     .get_value_mut()
        //     .step
        //     .poll(all_channel_waker.clone())
        //     .unwrap()
        // {
        //     StepPoll::NeedsState => todo!(),
        //     StepPoll::Emitted(e) => (CallerChannelStatus::OriginalStepFuture(self), Some(e)),
        //     StepPoll::Pending => (CallerChannelStatus::OriginalStepFuture(self), None),
        //     StepPoll::Ready => (
        //         CallerChannelStatus::Free(Free {
        //             caller_channel: self.caller_channel.vacate().0,
        //             steps: self.step.into_collection_mut(),
        //             blocked_source_channels: todo!(),
        //             repeat_step_blocked_reasons: todo!(),
        //             original_step_blocked_reasons: todo!(),
        //         }),
        //         None,
        //     ),
        // }
        todo!()
    }

    pub fn time(&self) -> T::Time {
        todo!()
    }
}