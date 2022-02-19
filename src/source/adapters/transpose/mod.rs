mod output_buffer;
mod storage;
mod transpose_inner;
mod transpose_metadata;

use std::pin::Pin;
use std::sync::Weak;
use std::task::{Context, Poll};

use futures_core::Future;
use pin_project::pin_project;

use self::transpose_inner::{PollResult, TransposeInner};
use crate::source::source_poll::SourcePollOk;
use crate::source::traits::SourceContext;
use crate::source::{Source, SourcePoll};
use crate::transposer::step::{Metadata, StepPoll, StepPollResult};
use crate::transposer::Transposer;
use crate::util::replace_waker::ReplaceWaker;
use crate::util::stack_waker::StackWaker;

#[pin_project(project=TransposeProject)]
pub struct Transpose<Src: Source, T: Transposer> {
    #[pin]
    source:             Src,
    source_waker:       Weak<ReplaceWaker>,
    latest_polled_time: T::Time,
    inner:              TransposeInner<T>,
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
            source_waker: ReplaceWaker::new_empty(),
            latest_polled_time: Default::default(),
            inner: TransposeInner::new(transposer, rng_seed),
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
        mut self: Pin<&mut Self>,
        poll_time: Self::Time,
        cx: SourceContext,
    ) -> SourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
        let TransposeProject {
            mut source,
            source_waker,
            inner,
            latest_polled_time,
        } = self.as_mut().project();

        let SourceContext {
            channel,
            one_channel_waker,
            all_channel_waker,
        } = cx;

        // first, poll for events if our source's all channel waker was called.
        if let Some(new_waker) = ReplaceWaker::register(source_waker, all_channel_waker.clone()) {
            let new_source_context = SourceContext {
                channel:           0,
                all_channel_waker: new_waker.clone(),
                one_channel_waker: new_waker.clone(),
            };

            loop {
                let events_poll = source
                    .as_mut()
                    .poll_events(*latest_polled_time, new_source_context.clone());

                match inner.handle_source_poll(events_poll) {
                    PollResult::Pending => return Poll::Pending,
                    PollResult::Continue => continue,
                    PollResult::Ready(s) => s,
                };
                break
            }
        }

        /*
            Procedure:

        E   1: return anything in the output buffer.
        P   2: check if we're already waiting, return pending if so
            3: get step
                a: UNSATURATED - NOT POSSIBLE
                b: SATURATING
                    1: poll step and handle
                        a: Pending
        P                   - return pending
                        b: NeedsState
                            - poll source for state
        P                       - return pending
                                - set state if ready
                        c: Ready
                            - ensure next is present
                            - start saturation of next
                c: SATURATED
                    1: ensure interpolation for channel and time is present
                    2: poll interpolation
        P               a: return pending if pending
        R               b: return Ready or Scheduled if ready

        ^^ return value:
            P: Pending
            E: Event/Rollback/Finalize
            R: Ready/Scheduled
             : continue to next instruction or loop
        */
        'main: loop {
            // get the events from the output buffer, returning if anything is there, and continuing if no buffered outputs occur before or at t.
            match inner.poll_output_buffer(poll_time) {
                SourcePollOk::Rollback(t) => return Poll::Ready(Ok(SourcePollOk::Rollback(t))),
                SourcePollOk::Event(e, t) => return Poll::Ready(Ok(SourcePollOk::Event(e, t))),
                SourcePollOk::Finalize(t) => return Poll::Ready(Ok(SourcePollOk::Finalize(t))),
                _ => {},
            };

            let step = inner.get_working_step(poll_time);
            let step_time = step.raw_time();
            match step.get_metadata_mut() {
                Metadata::Unsaturated(()) => panic!(),
                Metadata::Saturating(metadata) => {
                    let waker = match StackWaker::register(
                        &mut metadata.stack_waker,
                        channel,
                        one_channel_waker.clone(),
                    ) {
                        Some(w) => w,
                        None => return Poll::Pending,
                    };

                    let StepPoll {
                        result,
                        outputs,
                    } = step.poll(waker.clone()).unwrap();

                    inner.buffer_output_events(step_time, outputs);

                    match result {
                        StepPollResult::Pending => return Poll::Pending,
                        StepPollResult::NeedsState => {
                            // these are the same values they were before, because we aren't in Ready variant.
                            let step = inner.get_working_step(poll_time);
                            let metadata = match step.get_metadata_mut() {
                                Metadata::Saturating(m) => m,
                                _ => unreachable!(),
                            };
                            let c = match metadata.polling_channel {
                                Some(c) => c,
                                None => {
                                    let c = channel * 2 + 2;
                                    metadata.polling_channel = Some(c);
                                    c
                                },
                            };
                            let cx = SourceContext {
                                channel:           c,
                                one_channel_waker: waker.clone(),
                                all_channel_waker: all_channel_waker.clone(),
                            };

                            let poll = source.as_mut().poll(step.raw_time(), cx);
                            let state = match inner.handle_source_poll(poll) {
                                PollResult::Pending => return Poll::Pending,
                                PollResult::Continue => continue,
                                PollResult::Ready(s) => s,
                            };

                            // these are the same values they were before, because we aren't in Ready variant.
                            let step = inner.get_working_step(poll_time);
                            let _ = step.set_input_state(state, &waker);
                        },
                        StepPollResult::Ready => {
                            inner.ensure_working_poll_next_invariant(poll_time)
                        },
                    }
                },
                Metadata::Saturated(()) => {
                    let mut interpolation_cx = Context::from_waker(&one_channel_waker);

                    'saturated: loop {
                        let mut interpolation = inner.prepare_for_interpolation(channel, poll_time);
                        let poll = interpolation.as_mut().poll(&mut interpolation_cx);

                        match poll {
                            Poll::Ready(state) => {
                                inner.clear_channel(channel);
                                return Poll::Ready(Ok(match inner.get_scheduled_time() {
                                    Some(t) => SourcePollOk::Scheduled(state, t),
                                    None => SourcePollOk::Ready(state),
                                }))
                            },
                            Poll::Pending => {
                                if interpolation.needs_state() {
                                    let src_cx = SourceContext {
                                        channel:           channel * 2 + 1,
                                        one_channel_waker: one_channel_waker.clone(),
                                        all_channel_waker: all_channel_waker.clone(),
                                    };

                                    let poll = source.as_mut().poll(poll_time, src_cx);
                                    let state = match inner.handle_source_poll(poll) {
                                        PollResult::Pending => return Poll::Pending,
                                        PollResult::Continue => continue 'main,
                                        PollResult::Ready(s) => s,
                                    };
                                    let interpolation =
                                        inner.prepare_for_interpolation(channel, poll_time);
                                    let _ = interpolation.set_state(state, &one_channel_waker);

                                    continue 'saturated
                                }
                                return Poll::Pending
                            },
                        }
                    }
                },
            }
        }
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

    fn advance(self: Pin<&mut Self>, time: Self::Time) {
        self.project().inner.handle_caller_advance(time)
    }

    fn max_channel(&self) -> std::num::NonZeroUsize {
        todo!()
    }

    fn release_channel(self: Pin<&mut Self>, channel: usize) {
        let mut this = self.project();
        for c in this.inner.handle_caller_release_channel(channel) {
            this.source.as_mut().release_channel(c)
        }
    }
}
