use core::fmt::Debug;
use std::cmp::Ordering;
use std::fmt;
use std::time::Duration;

#[derive(Copy, Clone, Debug)]
pub struct EventTimestamp {
    // this is the duration since the initialization of the owning container.
    pub time: Duration,
    // break ties on time with priority.
    pub priority: usize,
}

impl Ord for EventTimestamp {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.time != other.time {
            return self.time.cmp(&other.time);
        }

        self.priority.cmp(&other.priority).reverse()
    }
}

impl PartialOrd for EventTimestamp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for EventTimestamp {}

impl PartialEq for EventTimestamp {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time && self.priority == other.priority
    }
}

#[derive(Clone, Debug)]
pub struct Event<T: Clone> {
    // so we can add/remove these after creating them.
    pub id: usize,
    pub content: EventContent<T>,
}

#[derive(Clone)]
pub struct EventContent<T: Clone> {
    pub timestamp: EventTimestamp,
    pub payload: T,
}

impl<T: Debug + Clone> Debug for EventContent<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Point")
            .field("timestamp", &self.timestamp)
            .field("payload", &self.payload)
            .finish()
    }
}

impl<T: Clone> Ord for Event<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.content.timestamp != other.content.timestamp {
            return self.content.timestamp.cmp(&other.content.timestamp);
        }

        // there should be no two events with the same id.
        if self.id == other.id {
            panic!();
        }

        // fall back to the order the events are created.
        self.id.cmp(&other.id)
    }
}

impl<T: Clone> PartialOrd for Event<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Clone> Eq for Event<T> {}

impl<T: Clone> PartialEq for Event<T> {
    fn eq(&self, other: &Self) -> bool {
        if self.id == other.id {
            panic!();
        }
        // there should be no two events with the same id.
        false
    }
}

#[derive(Clone)]
pub enum ScheduleEvent<Ext: Clone, Int: Clone> {
    External(Event<Ext>),
    Internal(Event<Int>),
}

impl<Ext: Debug + Clone, Int: Debug + Clone> Debug for ScheduleEvent<Ext, Int> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScheduleEvent::External(e) => e.fmt(f),
            ScheduleEvent::Internal(e) => e.fmt(f),
        }
    }
}

impl<Ext: Clone, Int: Clone> ScheduleEvent<Ext, Int> {
    pub fn timestamp(&self) -> EventTimestamp {
        match self {
            ScheduleEvent::External(e) => e.content.timestamp,
            ScheduleEvent::Internal(e) => e.content.timestamp,
        }
    }

    pub fn id(&self) -> usize {
        match self {
            ScheduleEvent::External(e) => e.id,
            ScheduleEvent::Internal(e) => e.id,
        }
    }
}

impl<Ext: Clone, Int: Clone> Ord for ScheduleEvent<Ext, Int> {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.timestamp() != other.timestamp() {
            return self.timestamp().cmp(&other.timestamp());
        }

        // fall back to the order the events are created.
        self.id().cmp(&other.id())
    }
}

impl<Ext: Clone, Int: Clone> PartialOrd for ScheduleEvent<Ext, Int> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<Ext: Clone, Int: Clone> Eq for ScheduleEvent<Ext, Int> {}

impl<Ext: Clone, Int: Clone> PartialEq for ScheduleEvent<Ext, Int> {
    fn eq(&self, other: &Self) -> bool {
        if self.id() == other.id() {
            panic!();
        }
        // there should be no two events with the same id.
        false
    }
}
