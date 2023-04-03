use std::collections::HashMap;
use std::sync::Weak;

use transposer::schedule_storage::DefaultStorage;
use transposer::step::{Interpolation, NoInput, NoInputManager};
use transposer::Transposer;
use util::stack_waker::StackWaker;

mod free;
mod interpolation_future;
mod original_step_future;
mod repeat_step_future;

// manage the association of source and caller channels.
// own steps.
// own interpolations.
// own one_channel_wakers.
pub struct ChannelStatuses<T: Transposer<InputStateManager = NoInputManager>> {
    // These are all the currently pending operations, from the perspective of the caller.
    // They can be blocked due to pending source_state, step_future, or interpolation_future.
    pub blocked_caller_channels:
        HashMap</* caller_channel */ usize, CallerChannelBlockedReason<T>>,

    // these are the wakers currently registered to each repeat step.
    pub blocked_repeat_step_wakers: HashMap<
        /* step_id */ usize,
        (
            /* number of blocked caller channels */ usize,
            Weak<StackWaker>,
        ),
    >,
}

impl<T: Transposer<InputStateManager = NoInputManager>> ChannelStatuses<T> {
    pub fn new() -> Self {
        Self {
            blocked_caller_channels:    HashMap::new(),
            blocked_repeat_step_wakers: HashMap::new(),
        }
    }

    /// from the current state, get the status.
    ///
    /// this internally holds mutable refs to the ChannelStatuses
    pub fn get_channel_status(&mut self, caller_channel: usize) -> CallerChannelStatus<'_, T> {
        todo!()
    }
}

pub enum CallerChannelBlockedReason<T: Transposer<InputStateManager = NoInputManager>> {
    OriginalStep,
    RepeatStep {
        step_id: usize,
    },
    InterpolationFuture {
        interpolation: Interpolation<T, NoInput, DefaultStorage>,
        forget:        bool,
    },
    Poisioned,
}

struct InterpolationWrapper<T: Transposer<InputStateManager = NoInputManager>> {
    source_channel: usize,
    interpolation:  Interpolation<T, NoInput, DefaultStorage>,
}

/// this enum represents the current blocked status for a given channel.
/// it can move between statuses under various circumstances,
/// like being provided a source state, or a future polling ready.
pub enum CallerChannelStatus<'a, T: Transposer<InputStateManager = NoInputManager>> {
    Free(free::Free<'a, T>),
    InterpolationFuture(interpolation_future::InterpolationFuture<'a, T>),
    OriginalStepFuture(original_step_future::OriginalStepFuture<'a, T>),
    RepeatStepFuture(repeat_step_future::RepeatStepFuture<'a, T>),
    Limbo,
}
