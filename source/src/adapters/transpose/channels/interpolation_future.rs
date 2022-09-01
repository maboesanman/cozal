
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

use super::interpolation_source_state::InterpolationSourceState;
use super::{CallerChannelBlockedReason, StepBlockedReason, CallerChannelStatus};

pub struct InterpolationFuture<'a, T: Transposer> {
    // entries
    caller_channel: HashMapOccupiedEntry<'a, usize, CallerChannelBlockedReason<T>>,

    // extra
    blocked_source_channels: &'a mut BTreeMap<usize, ()>,
    blocked_repeat_steps: &'a mut HashMap<usize, StepBlockedReason>,
    blocked_original_step: &'a mut Option<StepBlockedReason>,
    repeat_step_wakers: &'a mut BTreeMap<usize, HashMap<usize, Waker>>
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
            blocked_source_channels,
            blocked_repeat_steps,
            blocked_original_step,
            repeat_step_wakers,
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
            blocked_repeat_steps,
            blocked_original_step,
            repeat_step_wakers,
        };

        Err(CallerChannelStatus::InterpolationSourceState::<T>(inner))
    }
}
