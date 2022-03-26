use std::collections::{BTreeSet, HashMap};
use std::task::Poll;

use super::channel_assignments::{CallerChannelBlockedReason, StepBlockedReason};
use super::steps::Steps;
use crate::source::traits::SourceContext;
use crate::transposer::Transposer;

pub enum BlockedStatus<'a, T: Transposer> {
    Free(Free<'a, T>),
    OriginalStepSourceState(OriginalStepSourceState<'a, T>),
    OriginalStepFuture(OriginalStepFuture<'a, T>),
    RepeatStepSourceState(RepeatStepSourceState<'a, T>),
    RepeatStepFuture(RepeatStepFuture<'a, T>),
    InterpolationSourceState(InterpolationSourceState<'a, T>),
    InterpolationFuture(InterpolationFuture<'a, T>),
}

pub struct Free<'a, T: Transposer> {
    // vacant entry for blocked caller channel
    blocked_caller_channels: &'a mut HashMap<usize, CallerChannelBlockedReason<T>>,
    caller_channel:          usize,

    // extra
    steps: &'a mut Steps<T>,
    blocked_source_channels: &'a mut BTreeSet<usize>,
    repeat_step_blocked_reasons: &'a mut HashMap<usize, StepBlockedReason>,
    original_step_blocked_reasons: &'a mut Option<StepBlockedReason>,
}

pub struct OriginalStepSourceState<'a, T: Transposer> {
    // occupied entry for the blocked caller channel
    blocked_caller_channels: &'a mut HashMap<usize, CallerChannelBlockedReason<T>>,
    caller_channel:          usize,

    // occupied entry for the blocked input source
    blocked_source_channels: &'a mut BTreeSet<usize>,
    source_channel:          usize,

    // occupied entry for the original step (always last)
    steps: &'a mut Steps<T>,

    // occupied entry for reason (always Some)
    original_step_blocked_reasons: &'a mut Option<StepBlockedReason>,

    // extra
    repeat_step_blocked_reasons: &'a mut HashMap<usize, StepBlockedReason>,
}

pub struct OriginalStepFuture<'a, T: Transposer> {
    // occupied entry for the blocked caller channel
    blocked_caller_channels: &'a mut HashMap<usize, CallerChannelBlockedReason<T>>,
    caller_channel:          usize,

    // occupied entry for the original step (always last)
    steps: &'a mut Steps<T>,

    // occupied entry for reason (always Some)
    original_step_blocked_reasons: &'a mut Option<StepBlockedReason>,

    // extra
    blocked_source_channels:     &'a mut BTreeSet<usize>,
    repeat_step_blocked_reasons: &'a mut HashMap<usize, StepBlockedReason>,
}

pub struct RepeatStepSourceState<'a, T: Transposer> {
    // occupied entry for the blocked caller channel
    blocked_caller_channels: &'a mut HashMap<usize, CallerChannelBlockedReason<T>>,
    caller_channel:          usize,

    // occupied entry for the blocked input source
    blocked_source_channels: &'a mut BTreeSet<usize>,
    source_channel:          usize,

    // occupied entry for the repeat step
    steps: &'a mut Steps<T>,
    step:  usize,

    // occupied entry for reason, also uses step
    repeat_step_blocked_reasons: &'a mut HashMap<usize, StepBlockedReason>,

    // extra
    original_step_blocked_reasons: &'a mut Option<StepBlockedReason>,
}

pub struct RepeatStepFuture<'a, T: Transposer> {
    // occupied entry for the blocked caller channel
    blocked_caller_channels: &'a mut HashMap<usize, CallerChannelBlockedReason<T>>,
    caller_channel:          usize,

    // occupied entry for the repeat step
    steps: &'a mut Steps<T>,
    step:  usize,

    // occupied entry for reason, also uses step
    repeat_step_blocked_reasons: &'a mut HashMap<usize, StepBlockedReason>,

    // extra
    blocked_source_channels:       &'a mut BTreeSet<usize>,
    original_step_blocked_reasons: &'a mut Option<StepBlockedReason>,
}

pub struct InterpolationSourceState<'a, T: Transposer> {
    // occupied entry for the blocked caller channel
    blocked_caller_channels: &'a mut HashMap<usize, CallerChannelBlockedReason<T>>,
    caller_channel:          usize,

    // occupied entry for the blocked input source
    blocked_source_channels: &'a mut BTreeSet<usize>,
    source_channel:          usize,

    // extra
    steps: &'a mut Steps<T>,
    repeat_step_blocked_reasons: &'a mut HashMap<usize, StepBlockedReason>,
    original_step_blocked_reasons: &'a mut Option<StepBlockedReason>,
}

pub struct InterpolationFuture<'a, T: Transposer> {
    // occupied entry for the blocked caller channel
    blocked_caller_channels: &'a mut HashMap<usize, CallerChannelBlockedReason<T>>,
    caller_channel:          usize,

    // extra
    steps: &'a mut Steps<T>,
    blocked_source_channels: &'a mut BTreeSet<usize>,
    repeat_step_blocked_reasons: &'a mut HashMap<usize, StepBlockedReason>,
    original_step_blocked_reasons: &'a mut Option<StepBlockedReason>,
}

impl<'a, T: Transposer> Free<'a, T> {
    pub fn poll(self) -> BlockedStatus<'a, T> {
        todo!()
    }
}

impl<'a, T: Transposer> OriginalStepFuture<'a, T> {
    pub fn poll(self) -> BlockedStatus<'a, T> {
        todo!()
    }
}

impl<'a, T: Transposer> OriginalStepSourceState<'a, T> {
    pub fn get_context_for_source_poll(&self) -> SourceContext {
        todo!()
    }
    pub fn provide_state(self) -> BlockedStatus<'a, T> {
        todo!()
    }
}

impl<'a, T: Transposer> RepeatStepFuture<'a, T> {
    pub fn poll(self) -> BlockedStatus<'a, T> {
        todo!()
    }
}

impl<'a, T: Transposer> RepeatStepSourceState<'a, T> {
    pub fn get_context_for_source_poll(&self) -> SourceContext {
        todo!()
    }
    pub fn provide_state(self) -> BlockedStatus<'a, T> {
        todo!()
    }
}

impl<'a, T: Transposer> InterpolationFuture<'a, T> {
    pub fn poll(self) -> BlockedStatus<'a, T> {
        todo!()
    }
}

impl<'a, T: Transposer> InterpolationSourceState<'a, T> {
    pub fn get_context_for_source_poll(&self) -> SourceContext {
        todo!()
    }
    pub fn provide_state(self) -> BlockedStatus<'a, T> {
        todo!()
    }
}
