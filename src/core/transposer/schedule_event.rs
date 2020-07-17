use crate::core::event::event::{Event, EventTimestamp};
use std::{cmp::Ordering, fmt::Debug};
use core::fmt;

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

    pub fn id(&self) -> u64 {
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
