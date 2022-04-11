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
use crate::source::adapters::transpose_redux::channel_statuses::CallerChannelStatus;
use crate::source::source_poll::SourcePollOk;
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

enum SourcePollToHandle {}

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
            mut source,
            all_channel_waker,
            events_poll_time,
            channel_statuses,
        } = self.project();

        let SourceContext {
            channel: caller_channel,
            one_channel_waker,
            all_channel_waker: caller_all_channel_waker,
        } = cx;

        let mut unhandled_event_info: Option<SourcePollOk<Src::Time, Src::Event, ()>> = None;

        // poll events if our all channel waker was triggered.
        if let Some(waker) = ReplaceWaker::register(all_channel_waker, caller_all_channel_waker) {
            match source.as_mut().poll_events(*events_poll_time, waker) {
                Poll::Ready(Ok(poll)) => unhandled_event_info = Some(poll),
                Poll::Ready(Err(poll)) => {
                    todo!(/* handle new event info, possibly modifying input buffer, channel status, and output buffer */)
                },
                Poll::Pending => return Poll::Pending,
            }
        }

        let handle_scheduled = |t: T::Time| todo!();

        // at this point we only need to poll the source if state is needed.
        // we are ready to start manipulating the status,
        // handling blockers as they arise.

        let mut status = channel_statuses.get_channel_status(caller_channel);

        loop {
            if let Some(_) = unhandled_event_info {
                unhandled_event_info = None;
                todo!();
            }

            match core::mem::replace(&mut status, CallerChannelStatus::Poisioned) {
                CallerChannelStatus::Poisioned => unreachable!(),
                CallerChannelStatus::Free(_) => todo!(),
                CallerChannelStatus::OriginalStepSourceState(inner_status) => {
                    let state = match inner_status.get_args_for_source_poll() {
                        Some((time, cx)) => match source.as_mut().poll(time, cx) {
                            Poll::Ready(Ok(SourcePollOk::Ready(s))) => s,
                            Poll::Ready(Ok(SourcePollOk::Scheduled(s, t))) => {
                                handle_scheduled(t);
                                s
                            },
                            Poll::Ready(Ok(poll)) => {
                                unhandled_event_info = Some(poll.supress_state());
                                continue
                            },
                            Poll::Ready(Err(p)) => todo!(),
                            Poll::Pending => break Poll::Pending,
                        },
                        None => break Poll::Pending,
                    };

                    // this provide state call will not poll the future.
                    let future_state = inner_status.provide_state(state, &one_channel_waker);

                    // now loop again, polling the future on the next pass.
                    status = CallerChannelStatus::OriginalStepFuture(future_state);
                    continue
                },
                CallerChannelStatus::OriginalStepFuture(_) => todo!(),
                CallerChannelStatus::RepeatStepSourceState(_) => todo!(),
                CallerChannelStatus::RepeatStepFuture(_) => todo!(),
                CallerChannelStatus::InterpolationSourceState(_) => todo!(),
                CallerChannelStatus::InterpolationFuture(_) => todo!(),
            }
        }
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
