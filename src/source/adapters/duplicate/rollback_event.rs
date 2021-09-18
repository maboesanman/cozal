use core::{cmp::Ordering, task::Poll};

use crate::source::{SourcePoll, source_poll::SourcePollOk};


pub enum RollbackEvent<Time: Ord + Copy, Event> {
    Event { time: Time, event: Event },
    Rollback { time: Time },

    // never stored; used for searching.
    Search { time: Time },
}

impl<Time: Ord + Copy, Event> Clone for RollbackEvent<Time, Event>
where
    Event: Clone,
{
    fn clone(&self) -> Self {
        match self {
            Self::Event { time, event } => Self::Event {
                time: *time,
                event: event.clone(),
            },
            Self::Rollback { time } => Self::Rollback { time: *time },
            Self::Search { time } => Self::Search { time: *time },
        }
    }
}

impl<Time: Ord + Copy, Event> RollbackEvent<Time, Event> {
    fn time(&self) -> &Time {
        match self {
            Self::Event { time, .. } => time,
            Self::Rollback { time, .. } => time,
            Self::Search { time, .. } => time,
        }
    }
}

impl<Time: Ord + Copy, Event> Ord for RollbackEvent<Time, Event> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Event { .. }, Self::Rollback { .. }) => Ordering::Greater,
            (Self::Rollback { .. }, Self::Event { .. }) => Ordering::Less,
            (Self::Rollback { .. }, Self::Search { .. }) => Ordering::Less,
            (Self::Search { .. }, Self::Rollback { .. }) => Ordering::Greater,
            (s, o) => s.time().cmp(o.time()),
        }
    }
}

impl<Time: Ord + Copy, Event> PartialOrd for RollbackEvent<Time, Event> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<Time: Ord + Copy, Event> Eq for RollbackEvent<Time, Event> {}

impl<Time: Ord + Copy, Event> PartialEq for RollbackEvent<Time, Event> {
    fn eq(&self, other: &Self) -> bool {
        matches!(self.cmp(other), Ordering::Equal)
    }
}

impl<Time: Ord + Copy, Event, State, Error> Into<SourcePoll<Time, Event, State, Error>>
    for RollbackEvent<Time, Event>
{
    fn into(self) -> SourcePoll<Time, Event, State, Error> {
        Poll::Ready(Ok(match self {
            Self::Event { time, event } => SourcePollOk::Event(event, time),
            Self::Rollback { time } => SourcePollOk::Rollback(time),
            Self::Search { .. } => unreachable!(),
        }))
    }
}