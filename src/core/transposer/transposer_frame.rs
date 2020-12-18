use super::{
    expire_handle::{ExpireHandle, ExpireHandleFactory},
    internal_scheduled_event::InternalScheduledEvent,
};
use super::{internal_scheduled_event::Source, transposer::Transposer};
use im::{HashMap, OrdSet};
use std::sync::{Arc, Weak};

#[derive(Clone)]
pub(super) struct TransposerFrame<T: Transposer> {
    pub transposer: T,
    pub internal: TransposerFrameInternal<T>,
}

impl<T: Transposer> TransposerFrame<T> {
    pub fn new(transposer: T) -> Self {
        Self {
            transposer,
            internal: TransposerFrameInternal::new(),
        }
    }
}

#[derive(Clone)]
pub(super) struct TransposerFrameInternal<T: Transposer> {
    pub current_source: Source<T>,
    pub current_source_scheduling_index: usize,
    // schedule and expire_handles
    pub schedule: OrdSet<Arc<InternalScheduledEvent<T>>>,
    pub expire_handles: HashMap<ExpireHandle, Weak<InternalScheduledEvent<T>>>,

    pub expire_handle_factory: ExpireHandleFactory,
}

impl<T: Transposer> TransposerFrameInternal<T> {
    pub fn new() -> Self {
        Self {
            current_source: Source::Init,
            current_source_scheduling_index: 0,
            schedule: OrdSet::new(),
            expire_handles: HashMap::new(),
            expire_handle_factory: ExpireHandleFactory::new(),
        }
    }

    pub fn set_source(&mut self, new_source: Source<T>) {
        if new_source <= self.current_source {
            panic!()
        } else {
            self.current_source = new_source;
        }
    }

    pub fn time(&self) -> T::Time {
        self.current_source.time()
    }

    pub fn schedule_event(&mut self, time: T::Time, payload: T::Scheduled) -> Result<(), &str> {
        if time < self.time() {
            return Err("new event cannot sort before current event");
        }

        let new_event = InternalScheduledEvent {
            source: self.current_source.clone(),
            source_index: self.current_source_scheduling_index,
            expire_handle: None,
            time,
            payload,
        };

        let new_event = Arc::new(new_event);

        self.schedule.insert(new_event);
        self.current_source_scheduling_index += 1;
        Ok(())
    }

    pub fn schedule_event_expireable(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<ExpireHandle, &str> {
        if time < self.time() {
            return Err("new event cannot sort before current event");
        }

        let handle = self.expire_handle_factory.next();

        let new_event = InternalScheduledEvent {
            source: self.current_source.clone(),
            source_index: self.current_source_scheduling_index,
            expire_handle: Some(handle),
            time,
            payload,
        };

        let new_event = Arc::new(new_event);
        let new_event_weak = Arc::downgrade(&new_event.clone());

        self.schedule.insert(new_event);
        self.expire_handles.insert(handle, new_event_weak);
        self.current_source_scheduling_index += 1;

        Ok(handle)
    }

    pub fn expire_event(&mut self, handle: ExpireHandle) -> Result<(), &str> {
        match self.expire_handles.get(&handle) {
            Some(weak) => {
                if let Some(arc) = weak.upgrade() {
                    self.schedule.remove(&arc);
                    Ok(())
                } else {
                    Err("expired")
                }
            }
            None => Err("invalid expire handle"),
        }
    }
}
