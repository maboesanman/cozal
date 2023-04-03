use std::collections::{BTreeMap, HashMap};
use std::pin::Pin;
use std::sync::Weak;
use std::task::{Context, Poll, Waker};

use futures_core::Future;
use transposer::step::{Interpolation, NoInputManager, StepPoll};
use transposer::Transposer;
use util::extended_entry::hash_map::OccupiedExtEntry as HashMapOccupiedEntry;
use util::stack_waker::StackWaker;

use super::{CallerChannelBlockedReason, CallerChannelStatus};
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

        let poll = match value {
            CallerChannelBlockedReason::InterpolationFuture {
                interpolation,
                forget,
            } => {
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
}
