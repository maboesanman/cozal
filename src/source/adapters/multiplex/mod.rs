mod affinity_map;
mod assignment_map;

use core::pin::Pin;
use core::task::{Poll, Waker};
use std::collections::{
    VecDeque
};

use pin_project::pin_project;

use self::affinity_map::AffinityMap;
use self::assignment_map::AssignmentMap;
use crate::source::adapters::multiplex::assignment_map::{Assignment, PollType};
use crate::source::traits::SourceContext;
use crate::source::{Source, SourcePoll};

type AsyncWaker = Waker;
type OutChannelID = usize;
type SrcChannelID = usize;

#[pin_project]
pub struct Multiplex<Src: Source> {
    #[pin]
    source:            Src,
    assigned_channels: AssignmentMap<Src::Time>,
    channel_affinity:  AffinityMap,
    pending_channels:  VecDeque<PendingPoll<Src::Time>>,
}

impl<Src: Source> Multiplex<Src> {
    pub fn new(source: Src) -> Self {
        let max_channels = source.max_channels();
        Self {
            source,
            assigned_channels: AssignmentMap::new(max_channels),
            channel_affinity: AffinityMap::new(max_channels),
            pending_channels: VecDeque::new(),
        }
    }

    fn poll_internal<F, S>(
        self: Pin<&mut Self>,
        poll_time: Src::Time,
        mut cx: SourceContext<'_, '_>,
        poll_fn: F,
        poll_type: PollType,
    ) -> SourcePoll<Src::Time, Src::Event, S, Src::Error>
    where
        F: Fn(
            Pin<&mut Src>,
            Src::Time,
            SourceContext<'_, '_>,
        ) -> SourcePoll<Src::Time, Src::Event, S, Src::Error>,
    {
        let this = self.project();
        let source: Pin<&mut Src> = this.source;
        let assigned_channels: &mut AssignmentMap<Src::Time> = this.assigned_channels;
        let channel_affinity: &mut AffinityMap = this.channel_affinity;
        let pending_channels: &mut VecDeque<PendingPoll<Src::Time>> = this.pending_channels;

        let out_channel = cx.poll_channel;
        // step one: check for existing assignment, use it or clear it.

        'main: loop {
            match assigned_channels.get_assigned_source_channel(out_channel) {
                Some(assignment) => {
                    break {
                        // full match, use existing assignment
                        if assignment.poll_type == poll_type && assignment.time == poll_time {
                            let mut new_cx = cx.re_borrow();
                            new_cx.change_channel(assignment.source_channel);
                            let result = poll_fn(source, poll_time, new_cx);
                            if result.is_ready() {
                                if let Some(pending) = pending_channels.pop_front() {
                                    assigned_channels.assign(pending.out_channel, Assignment {
                                        poll_type:      pending.poll_type,
                                        time:           pending.time,
                                        source_channel: assignment.source_channel,
                                    });
                                    pending.waker.wake();
                                } else {
                                    assigned_channels.unassign(assignment.source_channel);
                                }
                            }
                            result
                        // partial match, only use existing assignment if nothing is queued
                        } else {
                            match pending_channels.pop_front() {
                                Some(pending) => {
                                    assigned_channels.assign(pending.out_channel, Assignment {
                                        poll_type:      pending.poll_type,
                                        time:           pending.time,
                                        source_channel: assignment.source_channel,
                                    });
                                    pending.waker.wake();
                                    Poll::Pending
                                },
                                None => {
                                    let mut new_cx = cx.re_borrow();
                                    new_cx.change_channel(assignment.source_channel);
                                    let result = poll_fn(source, poll_time, new_cx);
                                    if result.is_pending() {
                                        assigned_channels.assign(out_channel, Assignment {
                                            poll_type,
                                            time: poll_time,
                                            source_channel: assignment.source_channel,
                                        })
                                    }
                                    result
                                },
                            }
                        }
                    }
                },
                None => {
                    let mut channel = None;

                    // use affiliated channel if open
                    if let Some(affinity) =
                        channel_affinity.get_affiliated_source_channel(out_channel)
                    {
                        if assigned_channels
                            .get_assigned_output_channel(affinity)
                            .is_none()
                        {
                            channel = Some(affinity);
                        }
                    }

                    // use any open channel
                    if channel.is_none() {
                        channel = assigned_channels.get_unassigned_source_channel();
                    }

                    match channel {
                        // assign channel and loop
                        Some(source_channel) => {
                            assigned_channels.assign(out_channel, Assignment {
                                poll_type,
                                time: poll_time,
                                source_channel,
                            });
                            channel_affinity.set_affiliation(source_channel, out_channel);
                            continue
                        },
                        // enqueue call and return pending
                        None => {
                            let new_pending = PendingPoll {
                                poll_type,
                                time: poll_time,
                                out_channel,
                                waker: cx.async_context.waker().clone(),
                            };
                            for pending in pending_channels.iter_mut() {
                                if pending.out_channel == out_channel {
                                    *pending = new_pending;
                                    break 'main Poll::Pending
                                }
                            }
                            pending_channels.push_back(new_pending);
                            break Poll::Pending
                        },
                    }
                },
            }
        }
    }
}

struct PendingPoll<Time> {
    poll_type:   PollType,
    out_channel: OutChannelID,
    time:        Time,
    waker:       AsyncWaker,
}

impl<Src: Source> Source for Multiplex<Src> {
    type Time = Src::Time;

    type Event = Src::Event;

    type State = Src::State;

    type Error = Src::Error;

    fn poll(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: SourceContext<'_, '_>,
    ) -> SourcePoll<Self::Time, Self::Event, Self::State, Src::Error> {
        self.poll_internal(time, cx, Src::poll, PollType::Poll)
    }

    fn poll_forget(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: SourceContext<'_, '_>,
    ) -> SourcePoll<Self::Time, Self::Event, Self::State, Src::Error> {
        self.poll_internal(time, cx, Src::poll_forget, PollType::PollForget)
    }

    fn poll_events(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: SourceContext<'_, '_>,
    ) -> SourcePoll<Self::Time, Self::Event, (), Src::Error> {
        self.poll_internal(time, cx, Src::poll_events, PollType::PollEvents)
    }
}
