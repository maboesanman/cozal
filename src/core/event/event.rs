pub use super::event_timestamp::EventTimestamp;
use core::fmt::Debug;
use std::cmp::Ordering;
use std::fmt;

#[derive(Clone)]
pub struct Event<T: Copy + Ord, P: Clone> {
    pub timestamp: T,
    pub payload: EventPayload<P>,
}

#[derive(Clone, Debug)]
pub enum EventPayload<T> {
    Payload(T),
    Rollback,
}

impl<T: Debug + Copy + Ord, P: Debug + Clone> Debug for Event<T, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Point")
            .field("timestamp", &self.timestamp)
            .field("payload", &self.payload)
            .finish()
    }
}

impl<T: Copy + Ord, P: Clone> Ord for Event<T, P> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.timestamp.cmp(&other.timestamp)
    }
}

impl<T: Copy + Ord, P: Clone> PartialOrd for Event<T, P> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Copy + Ord, P: Clone> Eq for Event<T, P> {}

impl<T: Copy + Ord, P: Clone> PartialEq for Event<T, P> {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp.eq(&other.timestamp)
    }
}
