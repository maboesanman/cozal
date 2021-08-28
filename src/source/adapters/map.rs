use core::pin::Pin;
use core::task::Context;
use pin_project::pin_project;

use crate::source::Source;
use crate::source::SourcePoll;
use crate::source::traits::SourceContext;

#[pin_project]
pub struct Map<const CHANNELS: usize, Src: Source<CHANNELS>, E, S> {
    #[pin]
    source: Src,

    event_transform: fn(Src::Event) -> Option<E>,
    state_transform: fn(Src::State) -> S,
}

impl<const CHANNELS: usize, Src: Source<CHANNELS>, E, S> Map<CHANNELS, Src, E, S> {
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
}

impl<
    const CHANNELS: usize,
    Src: Source<CHANNELS>,
    E,
    S
> Source<CHANNELS> for Map<CHANNELS, Src, E, S> {
    type Time = Src::Time;
    type Event = E;
    type State = S;

    fn poll(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: &mut SourceContext<'_, CHANNELS, Src::Time>,
    ) -> SourcePoll<Src::Time, E, S> {
        let mut this = self.project();
        let e_fn = this.event_transform;
        let s_fn = this.state_transform;

        loop {
            break match this.source.as_mut().poll(time, cx) {
                SourcePoll::Pending => SourcePoll::Pending,
                SourcePoll::Rollback(t) => SourcePoll::Rollback(t),
                SourcePoll::Event(e, t) => match e_fn(e) {
                    Some(e) => SourcePoll::Event(e, t),
                    None => continue
                },
                SourcePoll::Scheduled(s, t) => SourcePoll::Scheduled(s_fn(s), t),
                SourcePoll::Ready(s) => SourcePoll::Ready(s_fn(s)),
            }
        }
    }
}
