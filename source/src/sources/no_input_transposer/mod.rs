use std::collections::VecDeque;

use transposer::step::{NoInput, NoInputManager, Step};
use transposer::Transposer;

use self::channels::ChannelStatuses;
use crate::source_poll::TrySourcePoll;
use crate::Source;

mod channels;

pub struct NoInputTransposerSource<T: Transposer<InputStateManager = NoInputManager>> {
    steps: VecDeque<Step<T, NoInput>>,

    channel_statuses: ChannelStatuses<T>,
}

impl<T: Transposer<InputStateManager = NoInputManager>> Source for NoInputTransposerSource<T> {
    type Time = T::Time;

    type Event = T::OutputEvent;

    type State = T::OutputState;

    type Error = ();

    fn poll(
        &mut self,
        time: Self::Time,
        cx: crate::traits::SourceContext,
    ) -> TrySourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
        todo!()
    }

    fn poll_events(
        &mut self,
        time: Self::Time,
        all_channel_waker: std::task::Waker,
    ) -> TrySourcePoll<Self::Time, Self::Event, (), Self::Error> {
        todo!()
    }

    fn release_channel(&mut self, channel: usize) {
        todo!()
    }

    fn advance(&mut self, time: Self::Time) {
        todo!()
    }

    fn max_channel(&self) -> std::num::NonZeroUsize {
        todo!()
    }
}
