// mod caller_channel_status;
// mod channel_assignments;
mod channels;
mod input_buffer;
mod retention_policy;
mod steps;
mod storage;
// mod transpose_step_metadata;

use std::pin::Pin;
use std::sync::Weak;
use std::task::{Poll, Waker};

use pin_project::pin_project;
use transposer::Transposer;
use util::replace_waker::ReplaceWaker;
use util::stack_waker::StackWaker;

use self::channels::ChannelStatuses;
use self::input_buffer::InputBuffer;
use self::retention_policy::RetentionPolicy;
use self::steps::Steps;
use crate::adapters::transpose::channels::CallerChannelStatus;
use crate::source_poll::SourcePollOk;
use crate::traits::SourceContext;
use crate::{Source, SourcePoll};

#[pin_project(project=TransposeProject)]
pub struct Transpose<Src: Source, T: Transposer> {
    // the source we pull from
    #[pin]
    source: Src,

    last_scheduled: Option<T::Time>,

    // the all channel waker to keep up to date.
    all_channel_waker: Weak<ReplaceWaker>,

    // the time to use for poll_events calls.
    // this should be the time of the latest emitted state,
    // or the currently saturating/unsaturated "original" step,
    // whichever is later
    events_poll_time: T::Time,

    // current channel obligations
    channel_statuses: ChannelStatuses<T>,

    steps: Steps<T>,

    input_buffer: InputBuffer<T>,
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
            last_scheduled: None,
            all_channel_waker: ReplaceWaker::new_empty(),
            events_poll_time: T::Time::default(),
            channel_statuses: ChannelStatuses::new(),
            steps: Steps::new(transposer, rng_seed),
            input_buffer: InputBuffer::new(),
        }
    }

    fn ready_or_scheduled(
        &self,
        state: T::OutputState,
    ) -> SourcePollOk<T::Time, T::Output, T::OutputState> {
        // match (
        //     self.channel_statuses.get_scheduled_time(),
        //     self.last_scheduled,
        // ) {
        //     (None, None) => SourcePollOk::Ready(state),
        //     (None, Some(t)) => SourcePollOk::Scheduled(state, t),
        //     (Some(t), None) => SourcePollOk::Scheduled(state, t),
        //     (Some(t1), Some(t2)) => SourcePollOk::Scheduled(state, std::cmp::min(t1, t2)),
        // }
        todo!()
    }

    fn poll_inner(
        mut self: Pin<&mut Self>,
        poll_time: T::Time,
        cx: SourceContext,
        forget: bool,
    ) -> SourcePoll<T::Time, T::Output, T::OutputState, Src::Error> {
        let TransposeProject {
            mut source,
            last_scheduled,
            all_channel_waker,
            events_poll_time,
            channel_statuses,
            steps,
            input_buffer,
        } = self.as_mut().project();

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
                    panic!("source poll error")
                },
                Poll::Pending => return Poll::Pending,
            }
        }

        // at this point we only need to poll the source if state is needed.
        // we are ready to start manipulating the status,
        // handling blockers as they arise.

        let mut status = channel_statuses.get_channel_status(caller_channel);

        let all_channel_waker = ReplaceWaker::get_waker(all_channel_waker);

        'outer: loop {
            if let Some(poll) = unhandled_event_info.take() {
                // handle poll
                drop(poll);
                todo!(/* handle new event info, possibly modifying input buffer and channel status */)
            }

            'inner: loop {
                match core::mem::replace(&mut status, CallerChannelStatus::Limbo) {
                    CallerChannelStatus::Limbo => unreachable!(),
                    CallerChannelStatus::Free(inner_status) => {
                        status = inner_status.poll(forget, poll_time);
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
                            Poll::Ready(Ok(SourcePollOk::Ready(s))) => {
                                *last_scheduled = None;
                                s
                            },
                            Poll::Ready(Ok(SourcePollOk::Scheduled(s, t))) => {
                                *last_scheduled = Some(t);
                                s
                            },
                            Poll::Ready(Ok(poll)) => {
                                unhandled_event_info = Some(poll.supress_state());
                                continue 'outer
                            },
                            Poll::Ready(Err(_)) => panic!("source poll error"),
                            Poll::Pending => break 'outer Poll::Pending,
                        };

                        // this provide state call will not poll the future.
                        let inner_status = inner_status.provide_state(state);

                        // now loop again, polling the future on the next pass.
                        status = CallerChannelStatus::OriginalStepFuture(inner_status);
                    },
                    CallerChannelStatus::OriginalStepFuture(inner_status) => {
                        todo!()
                        // let t = inner_status.time();

                        // // get the first item, so it can be pulled if needed by poll
                        // // (if original completes it needs to make a new original future)
                        // let mut first = input_buffer.pop_first();

                        // let (s, output) = inner_status.poll(&all_channel_waker, &mut first);

                        // // if poll didn't need the input, put it back in the buffer
                        // if let Some((t, inputs)) = first {
                        //     input_buffer.extend_front(t, inputs)
                        // }

                        // // handle all the generated outputs
                        // if let Some(output) = output {
                        //     return Poll::Ready(Ok(SourcePollOk::Event(output, t)))
                        // }

                        // status = s;
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
                            Poll::Ready(Ok(SourcePollOk::Ready(s))) => {
                                *last_scheduled = None;
                                s
                            },
                            Poll::Ready(Ok(SourcePollOk::Scheduled(s, t))) => {
                                *last_scheduled = Some(t);
                                s
                            },
                            Poll::Ready(Ok(poll)) => {
                                unhandled_event_info = Some(poll.supress_state());
                                continue 'outer
                            },
                            Poll::Ready(Err(_)) => panic!("source poll error"),
                            Poll::Pending => break 'outer Poll::Pending,
                        };

                        // this provide state call will not poll the future.
                        let inner_status = inner_status.provide_state(state);

                        // now loop again, polling the future on the next pass.
                        status = CallerChannelStatus::RepeatStepFuture(inner_status);
                    },
                    CallerChannelStatus::RepeatStepFuture(inner_status) => {
                        status = match inner_status.poll(&one_channel_waker) {
                            Poll::Ready(status) => status,
                            Poll::Pending => break 'outer Poll::Pending,
                        };
                    },
                    CallerChannelStatus::InterpolationSourceState(mut inner_status) => {
                        let (source_channel, never_remebered) =
                            inner_status.get_args_for_source_poll(forget);

                        let cx = SourceContext {
                            channel:           source_channel,
                            one_channel_waker: one_channel_waker.clone(),
                            all_channel_waker: all_channel_waker.clone(),
                        };

                        let poll = if never_remebered {
                            source.as_mut().poll_forget(poll_time, cx)
                        } else {
                            source.as_mut().poll(poll_time, cx)
                        };

                        let state = match poll {
                            Poll::Ready(Ok(SourcePollOk::Ready(s))) => {
                                *last_scheduled = None;
                                s
                            },
                            Poll::Ready(Ok(SourcePollOk::Scheduled(s, t))) => {
                                *last_scheduled = Some(t);
                                s
                            },
                            Poll::Ready(Ok(poll)) => {
                                unhandled_event_info = Some(poll.supress_state());
                                continue 'outer
                            },
                            Poll::Ready(Err(_)) => panic!("source poll error"),
                            Poll::Pending => break 'outer Poll::Pending,
                        };

                        let inner_status = inner_status.provide_state(state);

                        // now loop again, polling the future on the next pass.
                        status = CallerChannelStatus::InterpolationFuture(inner_status);
                    },
                    CallerChannelStatus::InterpolationFuture(inner_status) => {
                        let output_state = match inner_status.poll(&one_channel_waker) {
                            Ok(Poll::Ready(output_state)) => output_state,
                            Ok(Poll::Pending) => break 'outer Poll::Pending,
                            Err(s) => {
                                status = s;
                                continue 'inner
                            },
                        };

                        break 'outer Poll::Ready(Ok(self.ready_or_scheduled(output_state)))
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
        self.poll_inner(poll_time, cx, false)
    }

    fn poll_forget(
        self: Pin<&mut Self>,
        poll_time: Self::Time,
        cx: SourceContext,
    ) -> SourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
        self.poll_inner(poll_time, cx, true)
    }

    fn poll_events(
        self: Pin<&mut Self>,
        poll_time: Self::Time,
        all_channel_waker: Waker,
    ) -> SourcePoll<Self::Time, Self::Event, (), Self::Error> {
        todo!(/* some variant of poll_inner? */)
    }

    fn advance(self: Pin<&mut Self>, time: Self::Time) {
        todo!(/*
            move the caller advance header, mark old steps for deletion
        */)
    }

    fn max_channel(&self) -> std::num::NonZeroUsize {
        // this works out mathematically, as
        self.source.max_channel()
    }

    fn release_channel(self: Pin<&mut Self>, channel: usize) {
        todo!(/*
            delete the entry from the channel statuses,
            and forward the held channel if it was exclusively held
        */)
    }
}
