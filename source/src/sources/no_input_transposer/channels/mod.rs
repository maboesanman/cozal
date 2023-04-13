use std::collections::HashMap;
use std::sync::Weak;

use transposer::schedule_storage::DefaultStorage;
use transposer::step::{Interpolation, NoInput, NoInputManager};
use transposer::Transposer;
use util::extended_entry::hash_map::get_occupied;
use util::stack_waker::StackWaker;

use self::free::Free;
use self::interpolation_future::InterpolationFuture;
use self::original_step_future::OriginalStepFuture;
use self::repeat_step_future::RepeatStepFuture;

pub mod free;
pub mod interpolation_future;
pub mod original_step_future;
pub mod repeat_step_future;

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

    pub fn get_pinned_times(&self) -> Vec<T::Time> {
        get_pinned_times(&self.blocked_caller_channels)
    }

    /// from the current state, get the status.
    ///
    /// this internally holds mutable refs to the ChannelStatuses
    pub fn get_channel_status(&mut self, caller_channel: usize) -> CallerChannelStatus<'_, T> {
        match get_occupied(&mut self.blocked_caller_channels, caller_channel) {
            Ok(occupied) => match occupied.get_value().inner {
                CallerChannelBlockedReasonInner::OriginalStep => {
                    CallerChannelStatus::OriginalStepFuture(OriginalStepFuture {
                        caller_channel:             occupied,
                        blocked_repeat_step_wakers: &mut self.blocked_repeat_step_wakers,
                    })
                },
                CallerChannelBlockedReasonInner::RepeatStep(step_id) => {
                    CallerChannelStatus::RepeatStepFuture(RepeatStepFuture {
                        wakers:         get_occupied(&mut self.blocked_repeat_step_wakers, step_id)
                            .unwrap(),
                        caller_channel: occupied,
                    })
                },
                CallerChannelBlockedReasonInner::InterpolationFuture(_) => {
                    CallerChannelStatus::InterpolationFuture(InterpolationFuture {
                        caller_channel:             occupied,
                        blocked_repeat_step_wakers: &mut self.blocked_repeat_step_wakers,
                    })
                },
                CallerChannelBlockedReasonInner::Poisioned => panic!(),
            },
            Err(vacant) => CallerChannelStatus::Free(Free {
                caller_channel:             vacant,
                blocked_repeat_step_wakers: &mut self.blocked_repeat_step_wakers,
            }),
        }
    }
}

pub struct CallerChannelBlockedReason<T: Transposer<InputStateManager = NoInputManager>> {
    pub poll_time: T::Time,
    pub inner:     CallerChannelBlockedReasonInner<T>,
}

pub enum CallerChannelBlockedReasonInner<T: Transposer<InputStateManager = NoInputManager>> {
    OriginalStep,
    RepeatStep(usize),
    InterpolationFuture(Interpolation<T, NoInput, DefaultStorage>),
    Poisioned,
}

impl<T: Transposer<InputStateManager = NoInputManager>> CallerChannelBlockedReason<T> {
    pub fn get_pinned_time(&self) -> Option<T::Time> {
        match self.inner {
            CallerChannelBlockedReasonInner::OriginalStep
            | CallerChannelBlockedReasonInner::RepeatStep(_) => Some(self.poll_time),
            _ => None,
        }
    }

    pub fn unwrap_repeat_step(&self) -> usize {
        match self.inner {
            CallerChannelBlockedReasonInner::RepeatStep(step_id) => step_id,
            _ => panic!(),
        }
    }
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

pub fn get_pinned_times<T: Transposer<InputStateManager = NoInputManager>>(
    blocked_caller_channels: &HashMap<usize, CallerChannelBlockedReason<T>>,
) -> Vec<T::Time> {
    blocked_caller_channels
        .values()
        .filter_map(CallerChannelBlockedReason::get_pinned_time)
        .collect()
}
