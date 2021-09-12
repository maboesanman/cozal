use std::{collections::HashMap, pin::Pin, task::{Context, Waker}};
use pin_project::pin_project;
use crate::source::{Source, SourcePoll};

type EventWaker = Waker;
type AsyncWaker = Waker;
type OutChannelID = usize;
type SrcChannelID = usize;

#[pin_project]
pub struct Multiplex<Src: Source>
{
    #[pin]
    source: Src,
    out_channel_statuses: HashMap<usize, OutChannelStatus<Src::Time>>,
    next_channel: usize,
}

enum OutChannelStatus<Time> {
    Free,
    AssignedPoll(Time, SrcChannelID),
    AssignedPollForget(Time, SrcChannelID),
    AssignedPollEvents(Time, SrcChannelID),
    PendingPoll(Time, AsyncWaker),
    PendingPollForget(Time, AsyncWaker),
    PendingPollEvents(Time, AsyncWaker),
}

impl<Src: Source> Source for Multiplex<Src>
{
    type Time = Src::Time;

    type Event = Src::Event;

    type State = Src::State;

    fn max_channels(&self) -> Option<std::num::NonZeroUsize> {
        None
    }

    fn poll(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: crate::source::traits::SourceContext<'_, '_>,
    ) -> SourcePoll<Self::Time, Self::Event, Self::State> {
        todo!()
    }
}