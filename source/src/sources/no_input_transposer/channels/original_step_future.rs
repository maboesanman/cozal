use std::collections::HashMap;
use std::sync::Weak;
use std::task::Waker;

use transposer::schedule_storage::DefaultStorage;
use transposer::step::{NoInput, NoInputManager, Step, StepPoll};
use transposer::Transposer;
use util::extended_entry::hash_map::OccupiedExtEntry as HashMapOccupiedEntry;
use util::stack_waker::StackWaker;

use super::free::Free;
use super::{get_pinned_times, CallerChannelBlockedReason};

pub struct OriginalStepFuture<'a, T: Transposer<InputStateManager = NoInputManager>> {
    // entries
    pub caller_channel:             HashMapOccupiedEntry<'a, usize, CallerChannelBlockedReason<T>>,
    // extra
    pub blocked_repeat_step_wakers:
        &'a mut HashMap</* step_id */ usize, (usize, Weak<StackWaker>)>,
}

impl<'a, T: Transposer<InputStateManager = NoInputManager>> OriginalStepFuture<'a, T> {
    pub fn poll(
        self,
        step: &mut Step<T, NoInput, DefaultStorage>,
        all_channel_waker: &Waker,
    ) -> OriginalStepPoll<'a, T> {
        let Self {
            caller_channel,
            blocked_repeat_step_wakers,
        } = self;

        let poll = step.poll(all_channel_waker).unwrap();

        match poll {
            StepPoll::Emitted(event) => OriginalStepPoll::OutputEvent(event),
            StepPoll::Pending => OriginalStepPoll::Pending,
            StepPoll::Ready => OriginalStepPoll::Free(Free {
                caller_channel: caller_channel.vacate().0,
                blocked_repeat_step_wakers,
            }),
        }
    }

    pub fn abandon(self) -> Free<'a, T> {
        let Self {
            caller_channel,
            blocked_repeat_step_wakers,
        } = self;

        Free {
            caller_channel: caller_channel.vacate().0,
            blocked_repeat_step_wakers,
        }
    }

    pub fn get_pinned_times(&self) -> Vec<T::Time> {
        get_pinned_times(self.caller_channel.get_collection_ref())
    }
}

pub enum OriginalStepPoll<'a, T: Transposer<InputStateManager = NoInputManager>> {
    OutputEvent(T::OutputEvent),
    Free(Free<'a, T>),
    Pending,
}
