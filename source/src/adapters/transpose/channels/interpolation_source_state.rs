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

use super::interpolation_future::InterpolationFuture;
use super::{CallerChannelBlockedReason, RepeatStepBlockedReason, StepBlockedReason};

pub struct InterpolationSourceState<'a, T: Transposer> {
    // entries
    pub caller_channel: HashMapOccupiedEntry<'a, usize, CallerChannelBlockedReason<T>>,
    pub source_channel: BTreeMapOccupiedEntry<'a, usize, ()>,

    // extra
    pub blocked_repeat_steps:  &'a mut HashMap<usize, RepeatStepBlockedReason>,
    pub blocked_original_step: &'a mut Option<StepBlockedReason>,
}

impl<'a, T: Transposer> InterpolationSourceState<'a, T> {
    // don't need time or waker because they're just passed through for interpolations.
    pub fn get_args_for_source_poll(
        &mut self,
        can_forget: bool,
    ) -> (/* source channel */ usize, /* forget */ bool) {
        match self.caller_channel.get_value_mut() {
            CallerChannelBlockedReason::InterpolationSourceState {
                source_channel,
                interpolation: _,
                forget,
            } => {
                *forget &= can_forget;
                (*source_channel, *forget)
            },
            _ => unreachable!("CallerChannelBlockedReason does not match CallerChannelStatus"),
        }
    }

    pub fn provide_state(self, state: T::InputState) -> InterpolationFuture<'a, T> {
        let Self {
            // entries
            mut caller_channel,
            source_channel,

            // extra
            blocked_repeat_steps,
            blocked_original_step,
        } = self;

        if let CallerChannelBlockedReason::InterpolationSourceState {
            interpolation, ..
        } = caller_channel.get_value_mut()
        {
            let x = interpolation.set_state(state);

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
            blocked_source_channels,
            blocked_repeat_steps,
            blocked_original_step,
        }
    }
}
