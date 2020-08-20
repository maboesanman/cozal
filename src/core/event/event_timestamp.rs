
use std::cmp::Ordering;
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