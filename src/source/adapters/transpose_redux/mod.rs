mod blocked_status;
mod channel_assignments;
mod input_buffer;
mod output_buffer;
mod steps;
mod storage;
// mod transpose_step_metadata;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::pin::Pin;
use std::sync::Weak;

use pin_project::pin_project;

use self::steps::Steps;
use crate::source::traits::SourceContext;
use crate::source::{Source, SourcePoll};
use crate::transposer::Transposer;
use crate::util::replace_waker::ReplaceWaker;

#[pin_project(project=TransposeProject)]
pub struct Transpose<Src: Source, T: Transposer> {
    // the source we pull from
    #[pin]
    source: Src,

    // the collection of steps.
    steps: Steps<T>,

    // the all channel waker to keep up to date.
    all_channel_waker: Weak<ReplaceWaker>,

    // the time to use for poll_events calls.
    // this should be the time of the latest emitted state,
    // or the currently saturating/unsaturated "original" step,
    // whichever is later
    events_poll_time: T::Time,

    // These are all the currently pending operations, from the perspective of the source.
    // BTreeMap<source_channel, assignment>
    source_channels: BTreeSet<usize>,

    // These are all the currently pending operations, from the perspective of the caller.
    // They can be blocked due to pending source, step, or interpolation.
    // Each caller_channel is present in no more than one of the following collections:

    // HashMap<caller_channel, step_id>
    repeat_step_blocked_callers: HashMap<usize, usize>,

    // Option<(HashSet<caller_channel>, source_channel)>
    original_step_blocked_callers: Option<(HashSet<usize>, usize)>,

    // HashMap<caller_channel, source_channel>
    interpolation_blocked_callers: HashMap<usize, usize>,
}

impl<Src, T> Transpose<Src, T>
where
    Src: Source,
    T::Time: Copy + Ord + Default + Unpin, // TODO remove once https://github.com/rust-lang/rust/issues/91985 is resolved.
    T: Transposer<Time = Src::Time, Input = Src::Event, InputState = Src::State>,
    T: Clone,
{
    pub fn new(source: Src, transposer: T, rng_seed: [u8; 32]) -> Self {
        todo!()
    }
}

impl<Src, T> Source for Transpose<Src, T>
where
    Src: Source,
    T::Time: Copy + Ord + Default + Unpin, // TODO remove once https://github.com/rust-lang/rust/issues/91985 is resolved.
    T: Transposer<Time = Src::Time, Input = Src::Event, InputState = Src::State>,
    T: Clone,
{
    type Time = T::Time;

    type Event = T::Output;

    type State = T::OutputState;

    type Error = Src::Error;

    fn poll(
        self: Pin<&mut Self>,
        poll_time: Self::Time,
        cx: SourceContext,
    ) -> SourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
        todo!()
    }

    fn poll_forget(
        self: Pin<&mut Self>,
        poll_time: Self::Time,
        cx: SourceContext,
    ) -> SourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
        todo!()
    }

    fn poll_events(
        self: Pin<&mut Self>,
        poll_time: Self::Time,
        cx: SourceContext,
    ) -> SourcePoll<Self::Time, Self::Event, (), Self::Error> {
        todo!()
    }

    fn advance(self: Pin<&mut Self>, time: Self::Time) {
        todo!()
    }

    fn max_channel(&self) -> std::num::NonZeroUsize {
        self.source.max_channel()
    }

    fn release_channel(self: Pin<&mut Self>, channel: usize) {
        todo!()
    }
}
