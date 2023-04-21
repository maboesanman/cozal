use std::sync::Weak;
use std::task::{Poll, Waker};

use transposer::schedule_storage::DefaultStorage;
use transposer::step::{NoInput, NoInputManager, Step, StepPoll};
use transposer::Transposer;
use util::extended_entry::hash_map::OccupiedExtEntry as HashMapOccupiedEntry;
use util::stack_waker::StackWaker;

use super::free::Free;
use super::CallerChannelBlockedReason;

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

    pub fn abandon(self) -> Free<'a, T> {
        let Self {
            caller_channel,
            mut wakers,
        } = self;

        let (num_blocked_caller_channels, _) = wakers.get_value_mut();

        let blocked_repeat_step_wakers = {
            if *num_blocked_caller_channels == 1 {
                wakers.vacate().0.into_collection_mut().0
            } else {
                *num_blocked_caller_channels -= 1;
                wakers.into_collection_mut()
            }
        };

        Free {
            caller_channel: caller_channel.vacate().0,
            blocked_repeat_step_wakers,
        }
    }
}
