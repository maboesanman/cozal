use core::fmt::Debug;
use std::cmp::Ordering;
use std::fmt;
use std::time::Duration;

#[derive(Copy, Clone, Debug)]
pub struct EventTimestamp {
    // this is the duration since the initialization of the owning container.
    pub time: Duration,
    // break ties on time with priority.
    pub priority: i8,
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

#[derive(Clone)]
pub struct Event<T: Clone> {
    pub timestamp: EventTimestamp,
    pub payload: EventPayload<T>,
}


#[derive(Clone, Debug)]
pub enum EventPayload<T> {
    Payload(T),
    Rollback,
}

impl<T: Debug + Clone> Debug for Event<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Point")
            .field("timestamp", &self.timestamp)
            .field("payload", &self.payload)
            .finish()
    }
}

impl<T: Clone> Ord for Event<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.timestamp.cmp(&other.timestamp)
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
        self.timestamp.eq(&other.timestamp)
    }
}

