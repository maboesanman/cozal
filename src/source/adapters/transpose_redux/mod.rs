// mod caller_channel_status;
// mod channel_assignments;
mod channel_statuses;
mod input_buffer;
mod output_buffer;
mod steps;
mod storage;
// mod transpose_step_metadata;

use std::pin::Pin;
use std::sync::Weak;
use std::task::{Poll, Waker};

use pin_project::pin_project;

use self::channel_statuses::ChannelStatuses;
use crate::source::traits::SourceContext;
use crate::source::{Source, SourcePoll};
use crate::transposer::Transposer;
use crate::util::replace_waker::ReplaceWaker;

#[pin_project(project=TransposeProject)]
pub struct Transpose<Src: Source, T: Transposer> {
    // the source we pull from
    #[pin]
    source: Src,

    // the all channel waker to keep up to date.
    all_channel_waker: Weak<ReplaceWaker>,

    // the time to use for poll_events calls.
    // this should be the time of the latest emitted state,
    // or the currently saturating/unsaturated "original" step,
    // whichever is later
    events_poll_time: T::Time,

    // current statuses. this contains most of the state.
    channel_statuses: ChannelStatuses<T>,
}

impl<Src, T> Transpose<Src, T>
where
    Src: Source,
    T::Time: Copy + Ord + Default + Unpin, // TODO remove once https://github.com/rust-lang/rust/issues/91985 is resolved.
    T: Transposer<Time = Src::Time, Input = Src::Event, InputState = Src::State>,
    T: Clone,
{
    pub fn new(source: Src, transposer: T, rng_seed: [u8; 32]) -> Self {
        Self {
            source,
            all_channel_waker: ReplaceWaker::new_empty(),
            events_poll_time: T::Time::default(),
            channel_statuses: ChannelStatuses::new(transposer, rng_seed),
        }
    }

    pub fn poll_inner(
        self: Pin<&mut Self>,
        poll_time: T::Time,
        cx: SourceContext,
    ) -> SourcePoll<T::Time, T::Output, T::OutputState, Src::Error> {
        let TransposeProject {
            source,
            all_channel_waker,
            events_poll_time,
            channel_statuses,
        } = self.project();
        let SourceContext {
            channel: caller_channel,
            one_channel_waker,
            all_channel_waker: caller_all_channel_waker,
        } = cx;

        if let Some(waker) = ReplaceWaker::register(all_channel_waker, caller_all_channel_waker) {
            match source.poll_events(*events_poll_time, waker) {
                Poll::Ready(_) => todo!(),
                Poll::Pending => todo!(),
            }
        }
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
        all_channel_waker: Waker,
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
