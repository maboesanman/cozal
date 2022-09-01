use std::collections::{BTreeMap, HashMap};
use std::task::Waker;

use transposer::Transposer;

use util::extended_entry::hash_map::{
    VacantExtEntry as HashMapVacantEntry,
};

use super::{CallerChannelBlockedReason, StepBlockedReason, CallerChannelStatus};

pub struct Free<'a, T: Transposer> {
    // entries
    caller_channel: HashMapVacantEntry<'a, usize, CallerChannelBlockedReason<T>>,

    // extra
    blocked_source_channels: &'a mut BTreeMap<usize, ()>,
    blocked_repeat_steps: &'a mut HashMap<usize, StepBlockedReason>,
    blocked_original_steps: &'a mut Option<StepBlockedReason>,
    repeat_step_wakers: &'a mut BTreeMap<usize, HashMap<usize, Waker>>
}

impl<'a, T: Transposer> Free<'a, T> {
    /// either OriginalStepFuture, RepeatStepFuture, or InterpolationFuture
    pub fn poll(self, forget: bool, time: T::Time) -> CallerChannelStatus<'a, T> {
        todo!()
        // let Self {
        //     caller_channel,
        //     steps,
        //     blocked_source_channels,
        //     repeat_step_blocked_reasons,
        //     original_step_blocked_reasons,
        // } = self;
        // loop {
        //     match steps.get_before_or_at(time).unwrap() {
        //         super::steps::BeforeStatus::SaturatedImmediate(step) => {
        //             let interpolation = step.step.interpolate(time).unwrap();
        //             let blocked_reason = CallerChannelBlockedReason::InterpolationFuture {
        //                 interpolation,
        //                 forget,
        //             };
        //             let caller_channel = caller_channel.occupy(blocked_reason);

        //             break CallerChannelStatus::InterpolationFuture(InterpolationFuture {
        //                 caller_channel,
        //                 steps,
        //                 blocked_source_channels,
        //                 repeat_step_blocked_reasons,
        //                 original_step_blocked_reasons,
        //             })
        //         },
        //         super::steps::BeforeStatus::SaturatedDistant(saturated, next) => {
        //             // OPTIMIZATION: determine whether to saturate_take or saturate_clone
        //             next.step.saturate_clone(&saturated.step);
        //         },
        //         super::steps::BeforeStatus::Saturating(step) => {
        //             // join the saturating.

        //             todo!()
        //         },
        //     }
        // }
    }

    /// if there's nothing to do, return the time for scheduled.
    pub fn poll_events(self, time: T::Time) -> Result<Option<T::Time>, CallerChannelStatus<'a, T>> {
        todo!()
    }
}