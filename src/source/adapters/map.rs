use core::num::NonZeroUsize;
use core::pin::Pin;
use core::task::Poll;

use pin_project::pin_project;

use crate::source::source_poll::SourcePollOk;
use crate::source::traits::SourceContext;
use crate::source::{Source, SourcePoll};

#[pin_project]
pub struct Map<Src: Source, E, S> {
    #[pin]
    source: Src,

    event_transform: fn(Src::Event) -> Option<E>,
    state_transform: fn(Src::State) -> S,
}

impl<Src: Source, E, S> Map<Src, E, S> {
    pub fn new(
        source: Src,
        event_transform: fn(Src::Event) -> Option<E>,
        state_transform: fn(Src::State) -> S,
    ) -> Self {
        Self {
            source,
            event_transform,
            state_transform,
        }
    }

    fn poll_internal<F, Sin, Sout, SFn>(
        self: Pin<&mut Self>,
        time: Src::Time,
        cx: SourceContext,
        poll_fn: F,
        state_fn: SFn,
    ) -> SourcePoll<Src::Time, E, Sout, Src::Error>
    where
        F: Fn(
            Pin<&mut Src>,
            Src::Time,
            SourceContext,
        ) -> SourcePoll<Src::Time, Src::Event, Sin, Src::Error>,
        SFn: Fn(Sin) -> Sout,
    {
        let mut this = self.project();
        let e_fn = this.event_transform;

        loop {
            break match poll_fn(this.source.as_mut(), time, cx.clone()) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
                Poll::Ready(Ok(result)) => Poll::Ready(Ok(match result {
                    SourcePollOk::Rollback(t) => SourcePollOk::Rollback(t),
                    SourcePollOk::Event(e, t) => match e_fn(e) {
                        Some(e) => SourcePollOk::Event(e, t),
                        None => continue,
                    },
                    SourcePollOk::Scheduled(s, t) => SourcePollOk::Scheduled(state_fn(s), t),
                    SourcePollOk::Ready(s) => SourcePollOk::Ready(state_fn(s)),
                    SourcePollOk::Finalize(t) => SourcePollOk::Finalize(t),
                })),
            }
        }
    }
}

impl<Src: Source, E, S> Source for Map<Src, E, S> {
    type Time = Src::Time;
    type Event = E;
    type State = S;
    type Error = Src::Error;

    fn poll(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: SourceContext,
    ) -> SourcePoll<Src::Time, E, S, Src::Error> {
        let state_transform = self.state_transform;
        self.poll_internal(time, cx, Src::poll, state_transform)
    }

    fn poll_forget(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: SourceContext,
    ) -> SourcePoll<Self::Time, Self::Event, Self::State, Src::Error> {
        let state_transform = self.state_transform;
        self.poll_internal(time, cx, Src::poll_forget, state_transform)
    }

    fn poll_events(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: SourceContext,
    ) -> SourcePoll<Self::Time, Self::Event, (), Src::Error> {
        self.poll_internal(time, cx, Src::poll_events, |()| ())
    }

    fn advance(self: Pin<&mut Self>, time: Src::Time) {
        self.project().source.advance(time)
    }

    fn max_channel(&self) -> NonZeroUsize {
        self.source.max_channel()
    }

    fn release_channel(self: Pin<&mut Self>, channel: usize) {
        let this = self.project();
        this.source.release_channel(channel)
    }
}
