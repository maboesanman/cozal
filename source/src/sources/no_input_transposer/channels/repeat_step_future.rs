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

pub struct RepeatStepFuture<'a, T: Transposer<InputStateManager = NoInputManager>> {
    // entries
    pub caller_channel: HashMapOccupiedEntry<'a, usize, CallerChannelBlockedReason<T>>,
    pub wakers: HashMapOccupiedEntry<'a, /* step_id */ usize, (usize, Weak<StackWaker>)>,
}

impl<'a, T: Transposer<InputStateManager = NoInputManager>> RepeatStepFuture<'a, T> {
    pub fn poll(
        self,
        step: &mut Step<T, NoInput, DefaultStorage>,
        one_channel_waker: &Waker,
    ) -> Poll<Free<'a, T>> {
        let Self {
            caller_channel,
            mut wakers,
        } = self;

        // register waker and poll for
        let poll = match StackWaker::register(
            &mut wakers.get_value_mut().1,
            *caller_channel.get_key(),
            one_channel_waker.clone(),
        ) {
            Some(waker) => step.poll(&waker).unwrap(),
            None => StepPoll::Pending,
        };

        match poll {
            StepPoll::Emitted(_) => panic!(),
            StepPoll::Pending => Poll::Pending,
            StepPoll::Ready => {
                let blocked_repeat_step_wakers = if wakers.get_value().0 == 1 {
                    wakers.vacate().0.into_collection_mut().0
                } else {
                    wakers.get_value_mut().0 -= 1;
                    wakers.into_collection_mut()
                };
                Poll::Ready(Free {
                    caller_channel: caller_channel.vacate().0,
                    blocked_repeat_step_wakers,
                })
            },
        }
    }
}
