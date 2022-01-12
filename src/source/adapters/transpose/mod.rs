use std::collections::{HashMap, VecDeque};
use std::pin::Pin;
use std::sync::Weak;
use std::task::Wake;

use pin_project::pin_project;

use crate::source::Source;
use crate::transposer::schedule_storage::ImRcStorage;
use crate::transposer::step::{PointerInterpolation, Step, StepMetadata};
use crate::transposer::Transposer;
use crate::util::replace_waker::ReplaceWaker;
use crate::util::stack_waker::StackWaker;

#[pin_project]
pub struct Transpose<Src: Source, T: Transposer> {
    #[pin]
    source:          Src,
    source_waker:    ReplaceWaker,
    steps:           VecDeque<StepWrapper<T>>,
    active_channels: HashMap<usize, ChannelData<T>>,
}

struct StepWrapper<T: Transposer> {
    step:           Step<T, ImRcStorage, Metadata>,
    events_emitted: bool,
}

struct Metadata;

impl<T: Transposer> StepMetadata<T, ImRcStorage> for Metadata {
    type Unsaturated = ();

    type Saturating = Weak<StackWaker>;

    type Saturated = ();

    fn new_unsaturated() -> Self::Unsaturated {}

    fn to_saturating(_metadata: Self::Unsaturated) -> Self::Saturating {
        Default::default()
    }

    fn to_saturated(metadata: Self::Saturating, _transposer: &T) -> Self::Saturated {
        if let Some(w) = metadata.upgrade() {
            w.wake_by_ref();
        }
    }
}

enum ChannelStatus<T: Transposer> {
    Saturating {
        stack_waker: StackWaker,
    },
    Interpolating {
        interpolation: PointerInterpolation<T>,
    },
}

impl<T: Transposer> StepWrapper<T> {
    pub fn new_init(transposer: T, rng_seed: [u8; 32]) -> Self {
        Self {
            step:           Step::new_init(transposer, rng_seed),
            events_emitted: false,
        }
    }
}

struct ChannelData<T: Transposer> {
    channel:    usize,
    last_time:  T::Time,
    step_index: usize,
    status:     ChannelStatus<T>,
}

impl<Src, T> Transpose<Src, T>
where
    Src: Source,
    T::Time: Copy + Ord + Default + Unpin, // TODO remove once https://github.com/rust-lang/rust/issues/91985 is resolved.
    T: Transposer<Time = Src::Time, Input = Src::Event, InputState = Src::State>,
    T: Clone,
{
    pub fn new(_source: Src, _transposer: T, _rng_seed: [u8; 32]) -> Self {
        // let mut steps = VecDeque::new();
        // steps.push_back(StepWrapper::new_init(transposer, rng_seed));
        // Self {
        //     source,
        //     source_waker_observer: WakerObserver::new_dummy(),
        //     steps,
        //     current_channels: HashMap::new(),
        // }

        todo!()
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
        _time: Self::Time,
        _cx: crate::source::traits::SourceContext,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
        todo!()
    }

    fn poll_forget(
        self: Pin<&mut Self>,
        _time: Self::Time,
        _cx: crate::source::traits::SourceContext,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
        todo!()
    }

    fn poll_events(
        self: Pin<&mut Self>,
        _time: Self::Time,
        _cx: crate::source::traits::SourceContext,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, (), Self::Error> {
        todo!()
    }

    fn advance(self: Pin<&mut Self>, _time: Self::Time) {
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
