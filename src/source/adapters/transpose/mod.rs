use std::collections::{HashMap, VecDeque};

use crate::source::Source;
use crate::transposer::step_group::StepGroup;
use crate::transposer::Transposer;
use crate::util::observing_waker::WakerObserver;

pub struct Transpose<Src: Source, T: Transposer> {
    source:                Src,
    source_waker_observer: WakerObserver,
    steps:                 VecDeque<StepGroup<T>>,
    current_channels:      HashMap<usize, usize>,
}

impl<Src, T> Transpose<Src, T>
where
    Src: Source,
    T: Transposer<Time = Src::Time, Input = Src::Event, InputState = Src::State>,
    T: Clone,
    T::Scheduled: Clone,
{
    pub fn new(source: Src, transposer: T, rng_seed: [u8; 32]) -> Self {
        let mut steps = VecDeque::new();
        steps.push_back(StepGroup::new_init(transposer, rng_seed));
        Self {
            source,
            source_waker_observer: WakerObserver::new_dummy(),
            steps,
            current_channels: HashMap::new(),
        }
    }
}

impl<Src, T> Source for Transpose<Src, T>
where
    Src: Source,
    T: Transposer<Time = Src::Time, Input = Src::Event, InputState = Src::State>,
    T: Clone,
{
    type Time = T::Time;

    type Event = T::Output;

    type State = T::OutputState;

    type Error = Src::Error;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        time: Self::Time,
        cx: crate::source::traits::SourceContext,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
        todo!()
    }

    fn poll_forget(
        self: std::pin::Pin<&mut Self>,
        time: Self::Time,
        cx: crate::source::traits::SourceContext,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
        todo!()
    }

    fn poll_events(
        self: std::pin::Pin<&mut Self>,
        time: Self::Time,
        cx: crate::source::traits::SourceContext,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, (), Self::Error> {
        todo!()
    }

    fn advance(self: std::pin::Pin<&mut Self>, time: Self::Time) {
        todo!()
    }

    fn max_channel(&self) -> std::num::NonZeroUsize {
        todo!()
    }
}
