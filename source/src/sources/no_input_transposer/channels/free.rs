use std::collections::HashMap;
use std::sync::Weak;

use transposer::schedule_storage::DefaultStorage;
use transposer::step::{Interpolation, NoInput, NoInputManager};
use transposer::Transposer;
use util::extended_entry::hash_map::{get_occupied, VacantExtEntry as HashMapVacantEntry};
use util::stack_waker::StackWaker;

use super::interpolation_future::InterpolationFuture;
use super::original_step_future::OriginalStepFuture;
use super::repeat_step_future::RepeatStepFuture;
use super::{get_pinned_times, CallerChannelBlockedReason, CallerChannelBlockedReasonInner};

pub struct Free<'a, T: Transposer<InputStateManager = NoInputManager>> {
    // entries
    pub caller_channel: HashMapVacantEntry<'a, usize, CallerChannelBlockedReason<T>>,

    // extra
    pub blocked_repeat_step_wakers:
        &'a mut HashMap</* step_id */ usize, (usize, Weak<StackWaker>)>,
}

impl<'a, T: Transposer<InputStateManager = NoInputManager>> Free<'a, T> {
    pub fn start_interpolation(
        self,
        interpolation: Interpolation<T, NoInput, DefaultStorage>,
        poll_time: T::Time,
    ) -> InterpolationFuture<'a, T> {
        let Self {
            caller_channel: vacant_channel,
            blocked_repeat_step_wakers,
        } = self;

        let new_blocked_reason = CallerChannelBlockedReason {
            inner: CallerChannelBlockedReasonInner::InterpolationFuture(interpolation),
            poll_time,
        };

        let occupied_channel = vacant_channel.occupy(new_blocked_reason);

        InterpolationFuture {
            caller_channel: occupied_channel,
            blocked_repeat_step_wakers,
        }
    }

    pub fn start_repeat_step(self, step_id: usize, poll_time: T::Time) -> RepeatStepFuture<'a, T> {
        let Self {
            caller_channel: vacant_channel,
            blocked_repeat_step_wakers,
        } = self;

        let wakers = match get_occupied(blocked_repeat_step_wakers, step_id) {
            Ok(mut occupied) => {
                occupied.get_value_mut().0 += 1;
                occupied
            },
            Err(vacant) => vacant.occupy((1, Weak::new())),
        };

        let new_blocked_reason = CallerChannelBlockedReason {
            inner: CallerChannelBlockedReasonInner::RepeatStep(step_id),
            poll_time,
        };

        let occupied_channel = vacant_channel.occupy(new_blocked_reason);

        RepeatStepFuture {
            caller_channel: occupied_channel,
            wakers,
        }
    }

    pub fn start_original_step(self, poll_time: T::Time) -> OriginalStepFuture<'a, T> {
        let Self {
            caller_channel: vacant_channel,
            blocked_repeat_step_wakers,
        } = self;

        let new_blocked_reason = CallerChannelBlockedReason {
            inner: CallerChannelBlockedReasonInner::OriginalStep,
            poll_time,
        };

        let occupied_channel = vacant_channel.occupy(new_blocked_reason);

        OriginalStepFuture {
            caller_channel: occupied_channel,
            blocked_repeat_step_wakers,
        }
    }

    pub fn get_pinned_times(&self) -> Vec<T::Time> {
        get_pinned_times(self.caller_channel.get_collection_ref())
    }
}
