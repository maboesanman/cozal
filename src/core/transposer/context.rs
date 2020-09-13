use crate::core::Event;

use super::{expire_handle::{ExpireHandle, ExpireHandleFactory}, Transposer, ScheduledEvent};
use std::{collections::HashMap, sync::Mutex, sync::atomic::AtomicBool, sync::atomic::Ordering};

pub struct InitContext<T: Transposer> {
    // this is really an AtomicNonZeroU64
    pub(super) new_events: Mutex<Vec<ScheduledEvent<T>>>,
    pub(super) emitted_events: Mutex<Vec<T::Output>>,

    pub(super) expire_handle_factory: ExpireHandleFactory,
    pub(super) new_expire_handles: Mutex<HashMap<usize, ExpireHandle>>,
    // todo add seeded deterministic random function
}

impl<T: Transposer> InitContext<T> {
    pub(super) fn new(handle_factory: ExpireHandleFactory) -> Self {
        Self {
            new_events: Mutex::new(Vec::new()),
            emitted_events: Mutex::new(Vec::new()),
            expire_handle_factory: handle_factory,
            new_expire_handles: Mutex::new(HashMap::new()),
        }
    }

    pub(super) fn destroy(self) -> (
        Vec<ScheduledEvent<T>>,
        Vec<T::Output>,
        ExpireHandleFactory,
        HashMap<usize, ExpireHandle>,
    ) {
        (
            self.new_events.into_inner().unwrap(),
            self.emitted_events.into_inner().unwrap(),
            self.expire_handle_factory,
            self.new_expire_handles.into_inner().unwrap(),
        )
    }

    pub fn schedule_event(&self, time: T::Time, payload: T::Scheduled) {
        let mut new_events = self.new_events.lock().unwrap();
        new_events.push(Event {
            timestamp: time,
            payload,
        });
    }

    pub fn schedule_event_expireable(&self, time: T::Time, payload: T::Scheduled) -> ExpireHandle {
        let mut new_events = self.new_events.lock().unwrap();
        let mut new_expire_handles = self.new_expire_handles.lock().unwrap();

        let index = new_events.len();
        let handle = self.expire_handle_factory.next();

        new_events.push(Event {
            timestamp: time,
            payload,
        });
        new_expire_handles.insert(index, handle);

        handle
    }

    pub fn emit_event(&self, payload: T::Output) {
        let mut emitted_events = self.emitted_events.lock().unwrap();
        emitted_events.push(payload);
    }
}

pub struct UpdateContext<T: Transposer> {
    // this is really an AtomicNonZeroU64
    pub(super) new_events: Mutex<Vec<ScheduledEvent<T>>>,
    pub(super) emitted_events: Mutex<Vec<T::Output>>,
    pub(super) expired_events: Mutex<Vec<ExpireHandle>>,

    pub(super) expire_handle_factory: ExpireHandleFactory,
    pub(super) new_expire_handles: Mutex<HashMap<usize, ExpireHandle>>,
    // todo add seeded deterministic random function

    pub(super) exit: AtomicBool,
}

impl<T: Transposer> UpdateContext<T> {
    pub(super) fn new(handle_factory: ExpireHandleFactory) -> Self {
        Self {
            new_events: Mutex::new(Vec::new()),
            emitted_events: Mutex::new(Vec::new()),
            expired_events: Mutex::new(Vec::new()),
            expire_handle_factory: handle_factory,
            new_expire_handles: Mutex::new(HashMap::new()),
            exit: AtomicBool::new(false),
        }
    }

    pub(super) fn destroy(self) -> (
        Vec<ScheduledEvent<T>>,
        Vec<T::Output>,
        Vec<ExpireHandle>,
        ExpireHandleFactory,
        HashMap<usize, ExpireHandle>,
        bool,
    ) {
        (
            self.new_events.into_inner().unwrap(),
            self.emitted_events.into_inner().unwrap(),
            self.expired_events.into_inner().unwrap(),
            self.expire_handle_factory,
            self.new_expire_handles.into_inner().unwrap(),
            self.exit.into_inner(),
        )
    }

    pub fn schedule_event(&self, time: T::Time, payload: T::Scheduled) {
        let mut new_events = self.new_events.lock().unwrap();
        new_events.push(Event {
            timestamp: time,
            payload,
        });
    }

    pub fn schedule_event_expireable(&self, time: T::Time, payload: T::Scheduled) -> ExpireHandle {
        let mut new_events = self.new_events.lock().unwrap();
        let mut new_expire_handles = self.new_expire_handles.lock().unwrap();

        let index = new_events.len();
        let handle = self.expire_handle_factory.next();

        new_events.push(Event {
            timestamp: time,
            payload,
        });
        new_expire_handles.insert(index, handle);

        handle
    }

    pub fn emit_event(&self, payload: T::Output) {
        let mut emitted_events = self.emitted_events.lock().unwrap();
        emitted_events.push(payload);
    }

    pub fn expire_event(&self, handle: ExpireHandle) {
        let mut expired_events = self.expired_events.lock().unwrap();
        expired_events.push(handle);
    }

    pub fn exit(&self) {
        self.exit.fetch_or(true, Ordering::SeqCst);
    }
}