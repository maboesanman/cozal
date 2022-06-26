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
use self::input_buffer::InputBuffer;
use self::output_buffer::OutputBuffer;
use crate::source::adapters::transpose_redux::channel_statuses::CallerChannelStatus;
use crate::source::source_poll::SourcePollOk;
use crate::source::traits::SourceContext;
use crate::source::{Source, SourcePoll};
use crate::transposer::Transposer;
use crate::util::replace_waker::ReplaceWaker;
use crate::util::stack_waker::StackWaker;

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

    input_buffer: InputBuffer<T>,

    output_buffer: OutputBuffer<T>,
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
            input_buffer: InputBuffer::new(),
            output_buffer: OutputBuffer::new(),
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
            input_buffer,
            output_buffer,
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
                Poll::Ready(Err(_err)) => {
                    todo!(/* not sure what we're doing with errors */)
                },
                Poll::Pending => return Poll::Pending,
            }
        }

        let handle_scheduled = |_t: T::Time| todo!(/* record the scheduled time somehow */);

        // at this point we only need to poll the source if state is needed.
        // we are ready to start manipulating the status,
        // handling blockers as they arise.

        let mut status = channel_statuses.get_channel_status(caller_channel);

        let all_channel_waker = ReplaceWaker::get_waker(all_channel_waker);

        'outer: loop {
            if let Some(poll) = unhandled_event_info {
                unhandled_event_info = None;
                // handle poll
                drop(poll);
                todo!(/* handle new event info, possibly modifying input buffer, channel status, and output buffer */)
            }

            'inner: loop {
                match core::mem::replace(&mut status, CallerChannelStatus::Limbo) {
                    CallerChannelStatus::Limbo => unreachable!(),
                    CallerChannelStatus::Free(inner_status) => {
                        // TODO THIS NEEDS TO ACCOUNT FOR FORGET / EVENTS
                        status = inner_status.poll(false, poll_time);
                        continue 'inner
                    },
                    CallerChannelStatus::OriginalStepSourceState(mut inner_status) => {
                        let (time, source_channel) = inner_status.get_args_for_source_poll();

                        // original steps can emit events which effect all channels,
                        // so this uses the all channel waker for both of these.
                        let cx = SourceContext {
                            channel:           source_channel,
                            one_channel_waker: all_channel_waker.clone(),
                            all_channel_waker: all_channel_waker.clone(),
                        };

                        let state = match source.as_mut().poll(time, cx) {
                            Poll::Ready(Ok(SourcePollOk::Ready(s))) => s,
                            Poll::Ready(Ok(SourcePollOk::Scheduled(s, t))) => {
                                handle_scheduled(t);
                                s
                            },
                            Poll::Ready(Ok(poll)) => {
                                unhandled_event_info = Some(poll.supress_state());
                                continue 'outer
                            },
                            Poll::Ready(Err(_)) => todo!(),
                            Poll::Pending => break 'outer Poll::Pending,
                        };

                        // this provide state call will not poll the future.
                        let inner_status = inner_status.provide_state(state, false);

                        // now loop again, polling the future on the next pass.
                        status = CallerChannelStatus::OriginalStepFuture(inner_status);
                        continue 'inner
                    },
                    CallerChannelStatus::OriginalStepFuture(inner_status) => {
                        let t = inner_status.time();
                        let (s, outputs) = inner_status.poll(&all_channel_waker);
                        for o in outputs {
                            output_buffer.handle_output_event(t, o);
                        }
                        status = s;
                    },
                    CallerChannelStatus::RepeatStepSourceState(mut inner_status) => {
                        let (time, stack_waker, source_channel) =
                            inner_status.get_args_for_source_poll();

                        let stacked_waker = match StackWaker::register(
                            stack_waker,
                            caller_channel,
                            one_channel_waker.clone(),
                        ) {
                            Some(w) => w,
                            None => break 'outer Poll::Pending,
                        };

                        let cx = SourceContext {
                            channel:           source_channel,
                            one_channel_waker: stacked_waker,
                            all_channel_waker: all_channel_waker.clone(),
                        };

                        let state = match source.as_mut().poll(time, cx) {
                            Poll::Ready(Ok(SourcePollOk::Ready(s))) => s,
                            Poll::Ready(Ok(SourcePollOk::Scheduled(s, t))) => {
                                handle_scheduled(t);
                                s
                            },
                            Poll::Ready(Ok(poll)) => {
                                unhandled_event_info = Some(poll.supress_state());
                                continue 'outer
                            },
                            Poll::Ready(Err(_)) => todo!(),
                            Poll::Pending => break 'outer Poll::Pending,
                        };

                        // this provide state call will not poll the future.
                        let inner_status = inner_status.provide_state(state, false);

                        // now loop again, polling the future on the next pass.
                        status = CallerChannelStatus::RepeatStepFuture(inner_status);
                        continue 'inner
                    },
                    CallerChannelStatus::RepeatStepFuture(inner_status) => {
                        let stacked_waker = match StackWaker::register(
                            todo!(), // stack_waker,
                            caller_channel,
                            one_channel_waker.clone(),
                        ) {
                            Some(w) => w,
                            None => break 'outer Poll::Pending,
                        };
                        // this needs to poll with the stack waker.
                        todo!();
                        // status = inner_status.poll(&one_channel_waker);
                    },
                    CallerChannelStatus::InterpolationSourceState(_) => todo!(),
                    CallerChannelStatus::InterpolationFuture(inner_status) => {
                        let output_state = match inner_status.poll(&one_channel_waker) {
                            Ok(Poll::Ready(output_state)) => output_state,
                            Ok(Poll::Pending) => break 'outer Poll::Pending,
                            Err(s) => {
                                status = s;
                                continue 'inner
                            },
                        };

                        todo!()
                    },
                }
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
