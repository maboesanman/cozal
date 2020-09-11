use core::fmt::Debug;
use std::cmp::Ordering;
use std::fmt;

pub struct Event<T: Copy + Ord, P> {
    pub timestamp: T,
    pub payload: P,
}

impl<T: Debug + Copy + Ord, P: Debug> Debug for Event<T, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Event")
            .field("timestamp", &self.timestamp)
            .field("payload", &self.payload)
            .finish()
    }
}

impl<T: Copy + Ord, P> Ord for Event<T, P> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.timestamp.cmp(&other.timestamp)
    }
}

impl<T: Copy + Ord, P> PartialOrd for Event<T, P> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Copy + Ord, P> Eq for Event<T, P> {}

impl<T: Copy + Ord, P> PartialEq for Event<T, P> {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp.eq(&other.timestamp)
    }
}
