use std::collections::{BTreeMap, HashMap};
use std::pin::Pin;
use std::sync::Weak;
use std::task::{Context, Poll, Waker};

use futures_core::Future;
use transposer::schedule_storage::DefaultStorage;
use transposer::step::{Interpolation, NoInput, NoInputManager, Step, StepPoll};
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

use super::free::Free;
use super::{CallerChannelBlockedReason, CallerChannelStatus};

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
}

pub enum OriginalStepPoll<'a, T: Transposer<InputStateManager = NoInputManager>> {
    OutputEvent(T::OutputEvent),
    Free(Free<'a, T>),
    Pending,
}
