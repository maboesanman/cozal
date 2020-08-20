use core::fmt::Debug;
use std::cmp::Ordering;
use std::fmt;
pub use super::event_timestamp::EventTimestamp;

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

