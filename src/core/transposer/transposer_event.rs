use super::transposer::Transposer;
use crate::core::event::event::Event;
use std::cmp::Ordering;
use std::{num::NonZeroU64, sync::Arc};

/// A struct representing an externally generated transposer event
pub struct ExternalTransposerEvent<T: Transposer> {
    /// The event becomes read only once we read it in from the stream and store it in an [`Arc`].
    ///
    /// the purpose of this [`Arc`] is that we never have to clone the payload,
    /// and it can just be moved around until it is dropped.
    pub event: Arc<Event<T::Time, T::External>>,
}

// TODO refactor so this is not clone, and make ExternalTransposerEvent have an owned Event instead of an Arc.
impl<T: Transposer> Clone for ExternalTransposerEvent<T> {
    fn clone(&self) -> Self {
        ExternalTransposerEvent {
            event: self.event.clone(),
        }
    }
}

impl<T: Transposer> PartialOrd for ExternalTransposerEvent<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.event.timestamp.cmp(&other.event.timestamp) {
            Ordering::Equal => None,
            ord => Some(ord),
        }
    }
}

impl<T: Transposer> ExternalTransposerEvent<T> {
    fn second_sort(&self, _other: &Self) -> Option<Ordering> {
        None
    }
}

// impl<T: Transposer> ExternalTransposerEvent<T>
// where T::External: PartialOrd {
//     fn second_sort(&self, other: &Self) -> Option<Ordering> {
//         self.event.payload.cmp(other.event.payload)
//     }
// }

impl<T: Transposer> Eq for ExternalTransposerEvent<T> {}

impl<T: Transposer> PartialEq for ExternalTransposerEvent<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.event, &other.event)
    }
}

/// A struct representing an internally generated transposer event
///
/// All these events were returned by calls to [`TransposerEvent::update`].
pub struct InternalTransposerEvent<T: Transposer> {
    pub(super) created_at: T::Time,

    // this is the index in the new_events array in the result of the update or init function that spawned this event.
    pub(super) index: usize,
    pub(super) expire_handle: Option<NonZeroU64>,

    /// The event becomes read only once we create it and store it in an [`Arc`].
    ///
    /// the purpose of this [`Arc`] is that we never have to clone the payload,
    /// and it can just be moved around until it is dropped.
    pub event: Arc<Event<T::Time, T::Internal>>,
}

impl<T: Transposer> Clone for InternalTransposerEvent<T> {
    fn clone(&self) -> Self {
        InternalTransposerEvent {
            created_at: self.created_at,
            index: self.index,
            expire_handle: None,
            event: self.event.clone(),
        }
    }
}

impl<T: Transposer> InternalTransposerEvent<T> {
    fn second_sort(&self, other: &Self) -> Ordering {
        match self.created_at.cmp(&other.created_at) {
            Ordering::Equal => self.index.cmp(&other.index),
            ord => ord,
        }
    }
}

impl<T: Transposer> Ord for InternalTransposerEvent<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.event.timestamp.cmp(&other.event.timestamp) {
            Ordering::Equal => self.second_sort(other),
            ord => ord,
        }
    }
}

impl<T: Transposer> PartialOrd for InternalTransposerEvent<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Transposer> Eq for InternalTransposerEvent<T> {}

impl<T: Transposer> PartialEq for InternalTransposerEvent<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.event, &other.event)
    }
}

/// This enum encodes all the events that a transposer update function
/// is expected to be able to respond to.
///
/// [`TransposerEvent`]s are cheap to clone, as both variants primarily hold
/// [`Arc`](std::sync::Arc)s of events, and can be individually cloned.
pub enum TransposerEvent<T: Transposer> {

    /// The event came from the input stream.
    External(ExternalTransposerEvent<T>),

    /// The event was scheduled by ['Transposer::update`]
    Internal(InternalTransposerEvent<T>),
}

impl<T: Transposer> TransposerEvent<T> {
    /// returns the timestamp of the underlying event
    pub fn timestamp(&self) -> T::Time {
        match self {
            Self::External(e) => e.event.timestamp,
            Self::Internal(e) => e.event.timestamp,
        }
    }

    fn enum_sort_order(&self) -> u8 {
        match self {
            Self::Internal(_) => 0,
            Self::External(_) => 1,
        }
    }
}

// order is mostly deterministic. input events with equal time are not orderable.
impl<T: Transposer> PartialOrd for TransposerEvent<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let cmp = self.timestamp().cmp(&other.timestamp());
        if cmp != Ordering::Equal {
            return Some(cmp);
        }

        let cmp = self.enum_sort_order().cmp(&other.enum_sort_order());
        if cmp != Ordering::Equal {
            return Some(cmp);
        }

        match (self, other) {
            (Self::External(s), Self::External(o)) => s.second_sort(o),
            (Self::Internal(s), Self::Internal(o)) => Some(s.second_sort(o)),
            _ => unreachable!(),
        }
    }
}

impl<T: Transposer> Eq for TransposerEvent<T> {}

impl<T: Transposer> PartialEq for TransposerEvent<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (TransposerEvent::External(s), TransposerEvent::External(o)) => s.eq(o),
            (TransposerEvent::Internal(s), TransposerEvent::Internal(o)) => s.eq(o),
            _ => false,
        }
    }
}
