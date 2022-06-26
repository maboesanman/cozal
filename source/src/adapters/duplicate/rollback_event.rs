use core::cmp::Ordering;

use crate::source_poll::SourcePollOk;

#[derive(Clone)]
pub enum RollbackEvent<Time: Ord + Copy, Event> {
    Event { time: Time, event: Event },
    Rollback { time: Time },
    Finalize { time: Time },

    // never stored; used for searching.
    Search { time: Time },
}

impl<Time: Ord + Copy, Event> RollbackEvent<Time, Event> {
    fn time(&self) -> &Time {
        match self {
            Self::Event {
                time, ..
            } => time,
            Self::Rollback {
                time,
            } => time,
            Self::Search {
                time,
            } => time,
            Self::Finalize {
                time,
            } => time,
        }
    }
}

impl<Time: Ord + Copy, Event> Ord for RollbackEvent<Time, Event> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (
                Self::Rollback {
                    time: s,
                },
                Self::Rollback {
                    time: o,
                },
            ) => s.cmp(o),
            (
                _,
                Self::Rollback {
                    ..
                },
            ) => Ordering::Greater,
            (
                Self::Rollback {
                    ..
                },
                _,
            ) => Ordering::Less,
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

impl<Time: Ord + Copy, Event, State> From<RollbackEvent<Time, Event>>
    for SourcePollOk<Time, Event, State>
{
    fn from(rollback_event: RollbackEvent<Time, Event>) -> Self {
        match rollback_event {
            RollbackEvent::Event {
                time,
                event,
            } => SourcePollOk::Event(event, time),
            RollbackEvent::Rollback {
                time,
            } => SourcePollOk::Rollback(time),
            RollbackEvent::Finalize {
                time,
            } => SourcePollOk::Finalize(time),
            RollbackEvent::Search {
                ..
            } => unreachable!(),
        }
    }
}
