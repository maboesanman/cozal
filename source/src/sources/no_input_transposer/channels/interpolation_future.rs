use std::collections::HashMap;
use std::sync::Weak;
use std::task::{Context, Poll, Waker};

use futures_core::Future;
use transposer::step::NoInputManager;
use transposer::Transposer;
use util::extended_entry::hash_map::OccupiedExtEntry as HashMapOccupiedEntry;
use util::stack_waker::StackWaker;

use super::{
    get_pinned_times,
    CallerChannelBlockedReason,
    CallerChannelBlockedReasonInner,
    CallerChannelStatus,
};
use crate::sources::no_input_transposer::channels::free::Free;

pub struct InterpolationFuture<'a, T: Transposer<InputStateManager = NoInputManager>> {
    // entries
    pub caller_channel: HashMapOccupiedEntry<'a, usize, CallerChannelBlockedReason<T>>,

    // extra
    pub blocked_repeat_step_wakers:
        &'a mut HashMap</* step_id */ usize, (usize, Weak<StackWaker>)>,
}

impl<'a, T: Transposer<InputStateManager = NoInputManager>> InterpolationFuture<'a, T> {
    pub fn poll(
        self,
        one_channel_waker: &Waker,
    ) -> (CallerChannelStatus<'a, T>, Poll<T::OutputState>) {
        let Self {
            mut caller_channel,
            blocked_repeat_step_wakers,
        } = self;

        let value = caller_channel.get_value_mut();

        let poll = match &mut value.inner {
            CallerChannelBlockedReasonInner::InterpolationFuture(interpolation) => {
                let mut cx = Context::from_waker(one_channel_waker);
                std::pin::pin!(interpolation).poll(&mut cx)
            },
            _ => panic!(),
        };

        let status = if poll.is_ready() {
            CallerChannelStatus::Free(Free {
                caller_channel: caller_channel.vacate().0,
                blocked_repeat_step_wakers,
            })
        } else {
            CallerChannelStatus::InterpolationFuture(InterpolationFuture {
                caller_channel,
                blocked_repeat_step_wakers,
            })
        };

        (status, poll)
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
