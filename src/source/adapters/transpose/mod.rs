use std::collections::{HashMap, VecDeque};
use std::pin::Pin;

use pin_project::pin_project;

use crate::source::Source;
use crate::transposer::schedule_storage::ImRcStorage;
use crate::transposer::step_group::StepGroup;
use crate::transposer::Transposer;
use crate::util::observing_waker::WakerObserver;

#[pin_project]
pub struct Transpose<Src: Source, T: Transposer> {
    #[pin]
    source:                Src,
    source_waker_observer: WakerObserver,
    steps:                 VecDeque<StepGroupWrapper<T>>,
    current_channels:      HashMap<usize, ChannelData<T>>,
}

struct StepGroupWrapper<T: Transposer> {
    step_group: StepGroup<T, ImRcStorage>,
}

impl<T: Transposer> StepGroupWrapper<T> {
    pub fn new_init(transposer: T, rng_seed: [u8; 32]) -> Self {
        Self {
            step_group: StepGroup::new_init(transposer, rng_seed),
        }
    }
}

struct ChannelData<T: Transposer> {
    channel:   usize,
    last_time: T::Time,
}

impl<Src, T> Transpose<Src, T>
where
    Src: Source,
    T: Transposer<Time = Src::Time, Input = Src::Event, InputState = Src::State>,
    T: Clone,
{
    pub fn new(source: Src, transposer: T, rng_seed: [u8; 32]) -> Self {
        let mut steps = VecDeque::new();
        steps.push_back(StepGroupWrapper::new_init(transposer, rng_seed));
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
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: crate::source::traits::SourceContext,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
        todo!()
    }

    fn poll_forget(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: crate::source::traits::SourceContext,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
        todo!()
    }

    fn poll_events(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: crate::source::traits::SourceContext,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, (), Self::Error> {
        todo!()
    }

    fn advance(self: Pin<&mut Self>, time: Self::Time) {
        todo!()
    }

    fn max_channel(&self) -> std::num::NonZeroUsize {
        todo!()
    }

    fn release_channel(self: Pin<&mut Self>, channel: usize) {
        let this = self.project();
        this.source.release_channel(channel)
    }
}
