use super::{expire_handle::ExpireHandle, transposer::Transposer};
use std::cmp::Ordering;
use std::sync::Arc;

/// A struct representing an internally generated transposer event
///
/// All these events were returned by calls to [`Transposer::update`].
pub(super) struct InternalScheduledEvent<T: Transposer> {
    pub source: Source<T>,
    pub source_index: usize,

    pub expire_handle: Option<ExpireHandle>,

    pub time: T::Time,
    pub payload: T::Scheduled,
}

impl<T: Transposer> Ord for InternalScheduledEvent<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        let sort = self.time.cmp(&other.time);
        let sort = match sort {
            Ordering::Equal => self.source.cmp(&other.source),
            ord => return ord,
        };
        let sort = match sort {
            Ordering::Equal => self.source_index.cmp(&other.source_index),
            ord => return ord,
        };
        sort
    }
}

impl<T: Transposer> PartialOrd for InternalScheduledEvent<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Transposer> Eq for InternalScheduledEvent<T> {}

impl<T: Transposer> PartialEq for InternalScheduledEvent<T> {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
            && self.source_index == other.source_index
            && self.source == other.source
    }
}

#[derive(Clone)]
pub(super) enum Source<T: Transposer> {
    Init,
    Input(T::Time),
    Schedule(Arc<InternalScheduledEvent<T>>),
}

impl<T: Transposer> Source<T> {
    pub fn time(&self) -> T::Time {
        match self {
            Self::Init => T::Time::default(),
            Self::Input(time) => *time,
            Self::Schedule(event) => event.time,
        }
    }
}

impl<T: Transposer> Ord for Source<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Init, Self::Init) => Ordering::Equal,
            (Self::Init, Self::Input(_)) => Ordering::Less,
            (Self::Init, Self::Schedule(_)) => Ordering::Less,
            (Self::Input(_), Self::Init) => Ordering::Greater,
            (Self::Schedule(_), Self::Init) => Ordering::Greater,
            (Self::Schedule(s), Self::Schedule(o)) => match s.time.cmp(&o.time) {
                Ordering::Equal => match s.source.cmp(&o.source) {
                    Ordering::Equal => s.source_index.cmp(&o.source_index),
                    ord => ord,
                },
                ord => ord,
            },
            (s, o) => s.time().cmp(&o.time()),
        }
    }
}

impl<T: Transposer> PartialOrd for Source<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Transposer> Eq for Source<T> {}

impl<T: Transposer> PartialEq for Source<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Init, Self::Init) => true,
            (Self::Input(s), Self::Input(o)) => s == o,
            (Self::Schedule(s), Self::Schedule(o)) => s == o,
            _ => false,
        }
    }
}
