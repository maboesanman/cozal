mod output_buffer;
mod storage;
mod transpose_inner;
mod transpose_metadata;

use std::pin::Pin;
use std::sync::Weak;
use std::task::Poll;

use pin_project::pin_project;

use self::transpose_inner::{PollResult, TransposeInner};
use crate::source::source_poll::SourcePollOk;
use crate::source::traits::SourceContext;
use crate::source::{Source, SourcePoll};
use crate::transposer::Transposer;
use crate::util::replace_waker::ReplaceWaker;

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

    fn poll_impl(
        mut self: Pin<&mut Self>,
        poll_time: T::Time,
        forget: bool,
        cx: SourceContext,
    ) -> SourcePoll<T::Time, T::Output, T::OutputState, Src::Error> {
        let TransposeProject {
            mut source,
            source_waker,
            inner,
            latest_polled_time,
        } = self.as_mut().project();

        // first, poll for events if our source's all channel waker was called.
        if let Some(new_waker) = ReplaceWaker::register(source_waker, cx.all_channel_waker.clone())
        {
            let new_source_context = SourceContext {
                channel:           0,
                all_channel_waker: new_waker.clone(),
                one_channel_waker: new_waker.clone(),
            };

            loop {
                let events_poll = source
                    .as_mut()
                    .poll_events(*latest_polled_time, new_source_context.clone());

                match inner.handle_source_poll(events_poll)? {
                    PollResult::Pending => return Poll::Pending,
                    PollResult::PollAgain => continue,
                    PollResult::Ready(()) => break,
                }
            }
        }

        // get the events from the output buffer, returning if anything is there, and continuing if no buffered outputs occur before or at t.
        match inner.poll_output_buffer(poll_time) {
            SourcePollOk::Rollback(t) => return Poll::Ready(Ok(SourcePollOk::Rollback(t))),
            SourcePollOk::Event(e, t) => return Poll::Ready(Ok(SourcePollOk::Event(e, t))),
            SourcePollOk::Finalize(t) => return Poll::Ready(Ok(SourcePollOk::Finalize(t))),
            _ => {},
        };

        self.poll_loop(poll_time, forget, cx)
    }

    fn poll_loop(
        mut self: Pin<&mut Self>,
        poll_time: T::Time,
        forget: bool,
        cx: SourceContext,
    ) -> SourcePoll<T::Time, T::Output, T::OutputState, Src::Error> {
        let TransposeProject {
            mut source,
            source_waker: _,
            inner,
            latest_polled_time: _,
        } = self.as_mut().project();
        let mut poll = Some(inner.poll::<Src::Error>(poll_time, forget, cx.clone())?);

        let poll_ok = 'main: loop {
            match poll.take().unwrap() {
                transpose_inner::InnerPoll::Output {
                    time,
                    output,
                } => break 'main SourcePollOk::Event(output, time),
                transpose_inner::InnerPoll::Pending => return Poll::Pending,
                transpose_inner::InnerPoll::NeedsState {
                    time,
                    channel,
                    callback,
                    one_channel_waker,
                    forget,
                } => {
                    let new_source_context = SourceContext {
                        channel,
                        all_channel_waker: cx.all_channel_waker.clone(),
                        one_channel_waker,
                    };

                    let input_state = 'poll_source: loop {
                        let result = if forget {
                            source
                                .as_mut()
                                .poll_forget(time, new_source_context.clone())
                        } else {
                            source.as_mut().poll(time, new_source_context.clone())
                        };

                        match inner.handle_source_poll(result)? {
                            PollResult::Pending => return Poll::Pending,
                            PollResult::PollAgain => continue 'poll_source,
                            PollResult::Ready(s) => break 'poll_source s,
                        };
                    };

                    poll = Some(callback(input_state)?);
                },
                transpose_inner::InnerPoll::Ready(state) => break 'main SourcePollOk::Ready(state),
                transpose_inner::InnerPoll::Scheduled(state, time) => {
                    break 'main SourcePollOk::Scheduled(state, time)
                },
            };
        };

        Poll::Ready(Ok(poll_ok))
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
        self.poll_impl(poll_time, false, cx)
    }

    fn poll_forget(
        self: Pin<&mut Self>,
        poll_time: Self::Time,
        cx: crate::source::traits::SourceContext,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
        self.poll_impl(poll_time, true, cx)
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
