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

use super::{
    CallerChannelBlockedReason,
    CallerChannelStatus,
    RepeatStepBlockedReason,
    StepBlockedReason,
};

pub struct RepeatStepFuture<'a, T: Transposer> {
    // entries
    pub caller_channel: HashMapOccupiedEntry<'a, usize, CallerChannelBlockedReason<T>>,
    pub block_reason:   HashMapOccupiedEntry<'a, usize, RepeatStepBlockedReason>,

    // extra
    pub blocked_source_channels: &'a mut BTreeMap<usize, ()>,
    pub blocked_original_steps:  &'a mut Option<StepBlockedReason>,
}

impl<'a, T: Transposer> RepeatStepFuture<'a, T> {
    pub fn poll(mut self, one_channel_waker: &Waker) -> Poll<CallerChannelStatus<'a, T>> {
        // let stack_waker = match self.block_reason.get_value_mut() {
        //     RepeatStepBlockedReason::Future {
        //         stack_waker,
        //     } => stack_waker,
        //     _ => unreachable!(),
        // };

        // let channel = *self.caller_channel.get_key();

        // let waker = match StackWaker::register(stack_waker, channel, one_channel_waker.clone()) {
        //     Some(w) => w,
        //     None => return Poll::Pending,
        // };

        // let result = self.step.get_value_mut().step.poll(waker).unwrap();

        // let Self {
        //     caller_channel,
        //     step,
        //     block_reason,
        //     blocked_source_channels,
        //     original_step_blocked_reasons,
        // } = self;

        // let next_status = match result {
        //     StepPoll::NeedsState => RepeatStepSourceState {
        //         caller_channel,
        //         step,
        //         block_reason,
        //         source_channel: get_first_vacant(blocked_source_channels).occupy(()),
        //         original_step_blocked_reasons,
        //     },
        //     StepPoll::Emitted(_) => unreachable!(),
        //     StepPoll::Pending => return Poll::Pending,
        //     StepPoll::Ready => {
        //         let (caller_channel, _) = caller_channel.vacate();

        //         return todo!()
        //         // Free {
        //         //     caller_channel,
        //         //     steps: todo!(),
        //         //     blocked_source_channels,
        //         //     repeat_step_blocked_reasons: todo!(),
        //         //     original_step_blocked_reasons,
        //         // }
        //     },
        // };

        // Poll::Ready(next_status)
        todo!()
    }
}
