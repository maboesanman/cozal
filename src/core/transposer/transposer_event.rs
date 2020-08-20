use crate::core::event::event::{Event, EventTimestamp};
use std::{cmp::Ordering, fmt::Debug};
use core::fmt;
use std::rc::Rc;
use super::transposer::Transposer;

#[derive(Clone)]
pub(super) struct InitialTransposerEvent<T: Transposer> {
    // this is the index in the new_events array in the result of the init function.
    pub index: usize,
    pub event: Event<T::Internal>,
}

#[derive(Clone)]
pub(super) struct ExternalTransposerEvent<T: Transposer> {
    // this is the index of the event in the external event stream.
    pub index: usize,
    pub event: Event<T::External>,
}

#[derive(Clone)]
pub(super) struct InternalTransposerEvent<T: Transposer> {
    pub parent: Rc<TransposerEvent<T>>,

    // this is the index in the new_events array in the result of the update function of parent.
    pub index: usize,
    pub event: Event<T::Internal>,
}

#[derive(Clone)]
pub(super) enum TransposerEvent<T: Transposer> {
    Initial(InitialTransposerEvent<T>),
    External(ExternalTransposerEvent<T>),
    Internal(InternalTransposerEvent<T>),
}

impl<T: Transposer> TransposerEvent<T> {
    pub(super) fn timestamp(&self) -> EventTimestamp {
        match self {
            Self::Initial(e) => e.event.timestamp,
            Self::External(e) => e.event.timestamp,
            Self::Internal(e) => e.event.timestamp,
        }
    }
    
    fn enum_sort_order(&self) -> u8 {
        match self {
            Self::Initial(_) => 0,
            Self::External(_) => 1,
            Self::Internal(_) => 2,
        }
    }
}

// impl<Ext: Debug + Clone, Int: Debug + Clone> Debug for TransposerEvent<T> {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         match self {
//             TransposerEvent::Initial(e) => f.debug_struct("TransposerEvent::Initial")
//                     .field("index", &e.index)
//                     .field("event", &e.event)
//                     .finish(),
//             TransposerEvent::External(e) => f.debug_struct("TransposerEvent::External")
//                     .field("index", &e.index)
//                     .field("event", &e.event)
//                     .finish(),
//             TransposerEvent::Internal(e) => f.debug_struct("TransposerEvent::Internal")
//                     .field("parent", &e.parent)
//                     .field("index", &e.index)
//                     .field("event", &e.event)
//                     .finish(),
//         }
//     }
// }

// events are sorted by time, then by several other properties to ensure the order is deterministic
impl<T: Transposer> Ord for TransposerEvent<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        let cmp = self.timestamp().cmp(&other.timestamp());
        if cmp != Ordering::Equal{ return cmp; }

        let cmp = self.enum_sort_order().cmp(&other.enum_sort_order());
        if cmp != Ordering::Equal{ return cmp; }

        match (self, other) {
            (Self::Initial(s), Self::Initial(o)) => s.index.cmp(&o.index),
            (Self::External(s), Self::External(o)) => s.index.cmp(&o.index),
            (Self::Internal(s), Self::Internal(o)) => {
                let parent_cmp = s.parent.cmp(&o.parent);
                if parent_cmp != Ordering::Equal {
                    parent_cmp
                } else {
                    s.index.cmp(&o.index)
                }
            },
            _ => panic!(),
        }
    }
}

impl<T: Transposer> PartialOrd for TransposerEvent<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Transposer> Eq for TransposerEvent<T> {}

impl<T: Transposer> PartialEq for TransposerEvent<T> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
