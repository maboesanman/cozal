use std::{pin::Pin, task::{Context, Waker}};
use pin_project::pin_project;
use crate::source::{SourcePoll, traits::MultiChannelSource};

type EventWaker = Waker;
type AsyncWaker = Waker;
type OutChannelID = usize;
type SrcChannelID = usize;

#[pin_project]
pub struct Multiplex<Src: MultiChannelSource, const NEW_CHANNELS: usize>
where
    [(); NEW_CHANNELS]: Sized,
{
    #[pin]
    source: Src,
    out_channel_statuses: [OutChannelStatus<Src::Time>; NEW_CHANNELS],
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

impl<Src: MultiChannelSource, const NEW_CHANNELS: usize> MultiChannelSource for Multiplex<Src, NEW_CHANNELS>
where
    [(); NEW_CHANNELS]: Sized,
{
    type Time = Src::Time;

    type Event = Src::Event;

    type State = Src::State;

    const CHANNELS: usize = NEW_CHANNELS;

    fn poll(
        self: Pin<&mut Self>,
        time: Self::Time,
        channel: usize,
        cx: &mut Context<'_>,
        event_waker: Waker,
    ) -> SourcePoll<Self::Time, Self::Event, Self::State> {
        if channel >= Self::CHANNELS {
            panic!()
        }

        let this = self.project();
        let source: Pin<&mut Src> = this.source;
        let out_channel_statuses: &mut [OutChannelStatus<Src::Time>; NEW_CHANNELS] = this.out_channel_statuses;

        if let OutChannelStatus::AssignedPoll(assigned_time, src_channel) =  out_channel_statuses[channel] {
            if assigned_time == time {
                match source.poll(time, channel, cx, event_waker) {
                    SourcePoll::Pending => return SourcePoll::Pending,
                    result => {
                        out_channel_statuses[channel] = OutChannelStatus::Free;
                        return result;
                    }
                }
            }
        }

        out_channel_statuses[channel] = OutChannelStatus::PendingPoll(time, cx.waker().clone());

        let mut available_channels = Src::CHANNELS;
        for s in out_channel_statuses {
            if let OutChannelStatus::Assigned(_, _) = s {
                available_channels -= 1;
            }
        };

        for i in [0..Self::CHANNELS] {
            // let corrected = 
        }

        todo!()
    }
}