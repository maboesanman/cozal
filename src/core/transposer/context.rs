use crate::core::Event;

use super::{
    expire_handle::{ExpireHandle, ExpireHandleFactory},
    ScheduledEvent, Transposer,
};
use std::{collections::HashMap, sync::atomic::AtomicBool, sync::atomic::Ordering, sync::Mutex};

/// This is the interface through which you can do a variety of functions in your transposer.
///
/// the primary feature is scheduling events,
/// though there are more methods to interact with the engine.
pub struct InitContext<T: Transposer> {
    // this is really an AtomicNonZeroU64
    new_events: Mutex<Vec<ScheduledEvent<T>>>,
    emitted_events: Mutex<Vec<T::Output>>,

    expire_handle_factory: ExpireHandleFactory,
    new_expire_handles: Mutex<HashMap<usize, ExpireHandle>>,
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

    pub(super) fn destroy(
        self,
    ) -> (
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

    /// This allows you to schedule events to happen in the future.
    /// As long as the time you supply is not less than the current time,
    /// the event can be scheduled.
    pub fn schedule_event(&self, time: T::Time, payload: T::Scheduled) -> Result<(), &str> {
        if time < T::Time::default() {
            return Err("time must be in the future");
        }

        let mut new_events = match self.new_events.lock() {
            Ok(a) => a,
            Err(_) => {
                return Err("internal error");
            }
        };

        new_events.push(Event {
            timestamp: time,
            payload,
        });
        Ok(())
    }

    /// The same behavior as [`schedule_event`], but now returning an [`ExpireHandle`]
    /// which can be stored and used to cancel the event in the future.
    pub fn schedule_event_expireable(
        &self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<ExpireHandle, &str> {
        if time < T::Time::default() {
            return Err("time must be in the future");
        }

        let mut new_events = match self.new_events.lock() {
            Ok(a) => a,
            Err(_) => {
                return Err("internal error");
            }
        };
        let mut new_expire_handles = match self.new_expire_handles.lock() {
            Ok(a) => a,
            Err(_) => {
                return Err("internal error");
            }
        };

        let index = new_events.len();
        let handle = self.expire_handle_factory.next();

        new_events.push(Event {
            timestamp: time,
            payload,
        });
        new_expire_handles.insert(index, handle);

        Ok(handle)
    }

    /// This allows you to emit events from your transposer. the event is emitted at the current time.
    ///
    /// If you need to emit an event in the future at a scheduled time, schedule an internal event that can emit
    /// your event when handled.
    pub fn emit_event(&self, payload: T::Output) {
        let mut emitted_events = self.emitted_events.lock().unwrap();
        emitted_events.push(payload);
    }
}

/// This is the interface through which you can do a variety of functions in your transposer.
///
/// the primary features are scheduling and expiring events,
/// though there are more methods to interact with the engine.
pub struct UpdateContext<T: Transposer> {
    time: T::Time,
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
    pub(super) fn new(time: T::Time, handle_factory: ExpireHandleFactory) -> Self {
        Self {
            time,
            new_events: Mutex::new(Vec::new()),
            emitted_events: Mutex::new(Vec::new()),
            expired_events: Mutex::new(Vec::new()),
            expire_handle_factory: handle_factory,
            new_expire_handles: Mutex::new(HashMap::new()),
            exit: AtomicBool::new(false),
        }
    }

    pub(super) fn destroy(
        self,
    ) -> (
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

    /// This allows you to schedule events to happen in the future.
    /// As long as the time you supply is not less than the current time,
    /// the event can be scheduled.
    pub fn schedule_event(&self, time: T::Time, payload: T::Scheduled) -> Result<(), &str> {
        if time < self.time {
            return Err("time must be in the future");
        }

        let mut new_events = match self.new_events.lock() {
            Ok(a) => a,
            Err(_) => {
                return Err("internal error");
            }
        };

        new_events.push(Event {
            timestamp: time,
            payload,
        });
        Ok(())
    }

    /// The same behavior as [`schedule_event`], but now returning an [`ExpireHandle`]
    /// which can be stored and used to cancel the event in the future.
    pub fn schedule_event_expireable(
        &self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<ExpireHandle, &str> {
        if time < self.time {
            return Err("time must be in the future");
        }

        let mut new_events = match self.new_events.lock() {
            Ok(a) => a,
            Err(_) => {
                return Err("internal error");
            }
        };
        let mut new_expire_handles = match self.new_expire_handles.lock() {
            Ok(a) => a,
            Err(_) => {
                return Err("internal error");
            }
        };

        let index = new_events.len();
        let handle = self.expire_handle_factory.next();

        new_events.push(Event {
            timestamp: time,
            payload,
        });
        new_expire_handles.insert(index, handle);

        Ok(handle)
    }

    /// This allows you to emit events from your transposer. the event is emitted at the current time.
    ///
    /// If you need to emit an event in the future at a scheduled time, schedule an internal event that can emit
    /// your event when handled.
    pub fn emit_event(&self, payload: T::Output) {
        let mut emitted_events = self.emitted_events.lock().unwrap();
        emitted_events.push(payload);
    }

    /// This allows you to expire an event currently in the schedule, as long as you have an [`ExpireHandle`].
    pub fn expire_event(&self, handle: ExpireHandle) {
        let mut expired_events = self.expired_events.lock().unwrap();
        expired_events.push(handle);
    }

    /// This allows you to exit the transposer, closing the output stream.
    pub fn exit(&self) {
        self.exit.fetch_or(true, Ordering::SeqCst);
    }
}
