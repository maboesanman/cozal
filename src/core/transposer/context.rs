use crate::core::Event;
use futures::channel::oneshot::{Receiver, Sender};
use std::{
    collections::HashMap,
    sync::{Arc, Weak},
};

use super::{
    expire_handle::{ExpireHandle, ExpireHandleFactory},
    internal_scheduled_event::{InternalScheduledEvent, Source},
    InternalOutputEvent, ScheduledEvent, Transposer,
};

/// This is the interface through which you can do a variety of functions in your transposer.
///
/// the primary feature is scheduling events,
/// though there are more methods to interact with the engine.
pub struct InitContext<T: Transposer> {
    // this is really an AtomicNonZeroU64
    new_events: Vec<ScheduledEvent<T>>,
    emitted_events: Vec<T::Output>,

    expire_handle_factory: ExpireHandleFactory,
    new_expire_handles: HashMap<usize, ExpireHandle>,
    // todo add seeded deterministic random function
}

impl<T: Transposer> InitContext<T> {
    pub(super) fn new(handle_factory: ExpireHandleFactory) -> Self {
        Self {
            new_events: Vec::new(),
            emitted_events: Vec::new(),
            expire_handle_factory: handle_factory,
            new_expire_handles: HashMap::new(),
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
            self.new_events,
            self.emitted_events,
            self.expire_handle_factory,
            self.new_expire_handles,
        )
    }

    /// This allows you to schedule events to happen in the future.
    /// As long as the time you supply is not less than the current time,
    /// the event can be scheduled.
    pub async fn schedule_event(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<(), &str> {
        if time < T::Time::default() {
            return Err("time must be in the future");
        }

        // let mut new_events = self.new_events.lock().await;

        self.new_events.push(Event {
            timestamp: time,
            payload,
        });

        Ok(())
    }

    /// The same behavior as [`schedule_event`], but now returning an [`ExpireHandle`]
    /// which can be stored and used to cancel the event in the future.
    pub async fn schedule_event_expireable(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<ExpireHandle, &str> {
        if time < T::Time::default() {
            return Err("time must be in the future");
        }

        let index = self.new_events.len();
        let handle = self.expire_handle_factory.next();

        self.new_events.push(Event {
            timestamp: time,
            payload,
        });
        self.new_expire_handles.insert(index, handle);

        Ok(handle)
    }

    /// This allows you to emit events from your transposer. the event is emitted at the current time.
    ///
    /// If you need to emit an event in the future at a scheduled time, schedule an internal event that can emit
    /// your event when handled.
    pub async fn emit_event(&mut self, payload: T::Output) {
        self.emitted_events.push(payload);
    }
}

pub(super) enum LazyState<S> {
    Ready(S),
    Pending(Receiver<S>),
}

impl<S> LazyState<S> {
    pub async fn get(&mut self) -> &S {
        match self {
            Self::Ready(s) => s,
            Self::Pending(r) => {
                let s = r.await.unwrap();
                std::mem::swap(self, &mut Self::Ready(s));
                if let Self::Ready(s) = self {
                    s
                } else {
                    unreachable!()
                }
            }
        }
    }

    pub fn destroy(self) -> Option<S> {
        match self {
            Self::Ready(s) => Some(s),
            Self::Pending(_) => None,
        }
    }
}

/// This is the interface through which you can do a variety of functions in your transposer.
///
/// the primary features are scheduling and expiring events,
/// though there are more methods to interact with the engine.
pub struct UpdateContext<'a, T: Transposer> {
    // internal tracking
    trigger: Source<T>,
    event_schedule_index: usize,

    // mutable references into the current transposer frame
    schedule: &'a mut im::OrdSet<Arc<InternalScheduledEvent<T>>>,
    expire_handles: &'a mut im::HashMap<ExpireHandle, Weak<InternalScheduledEvent<T>>>,
    expire_handle_factory: &'a mut ExpireHandleFactory,

    // access to the input state
    input_state: &'a mut LazyState<T::InputState>,
    pub(super) input_state_requester: Option<Sender<()>>,

    // values to output
    pub(super) output_events: Vec<InternalOutputEvent<T>>,
    pub(super) exit: bool,
}

impl<'a, T: Transposer> UpdateContext<'a, T> {
    pub(super) fn new_input(
        time: T::Time,
        // this can't be a mutable reference to the frame because the borrow needs to be split.
        schedule: &'a mut im::OrdSet<Arc<InternalScheduledEvent<T>>>,
        expire_handles: &'a mut im::HashMap<ExpireHandle, Weak<InternalScheduledEvent<T>>>,
        expire_handle_factory: &'a mut ExpireHandleFactory,
        input_state: &'a mut LazyState<T::InputState>,
    ) -> Self {
        Self {
            trigger: Source::Input(time),
            event_schedule_index: 0,

            schedule,
            expire_handles,
            expire_handle_factory,

            input_state,
            input_state_requester: None,

            output_events: Vec::new(),
            exit: false,
        }
    }

    pub(super) fn new_scheduled(
        event_arc: Arc<InternalScheduledEvent<T>>,
        schedule: &'a mut im::OrdSet<Arc<InternalScheduledEvent<T>>>,
        expire_handles: &'a mut im::HashMap<ExpireHandle, Weak<InternalScheduledEvent<T>>>,
        expire_handle_factory: &'a mut ExpireHandleFactory,
        input_state: &'a mut LazyState<T::InputState>,
        input_state_requester: Option<Sender<()>>,
    ) -> Self {
        Self {
            trigger: Source::Schedule(event_arc),
            event_schedule_index: 0,

            schedule,
            expire_handles,
            expire_handle_factory,

            input_state,
            input_state_requester,

            output_events: Vec::new(),
            exit: false,
        }
    }

    fn time(&self) -> T::Time {
        self.trigger.time()
    }

    pub async fn get_input_state(&mut self) -> Result<&T::InputState, &str> {
        if let Some(requester) = std::mem::take(&mut self.input_state_requester) {
            let _ = requester.send(());
        }
        Ok(self.input_state.get().await)
    }

    /// This allows you to schedule events to happen in the future.
    /// As long as the time you supply is not less than the current time,
    /// the event can be scheduled.
    pub fn schedule_event(&mut self, time: T::Time, payload: T::Scheduled) -> Result<(), &str> {
        if time < self.time() {
            return Err("new event cannot sort before current event");
        }

        let new_event = InternalScheduledEvent {
            source: self.trigger.clone(),
            source_index: self.event_schedule_index,
            expire_handle: None,
            time,
            payload,
        };

        let new_event = Arc::new(new_event);

        self.schedule.insert(new_event);
        self.event_schedule_index += 1;
        Ok(())
    }

    /// The same behavior as [`schedule_event`], but now returning an [`ExpireHandle`]
    /// which can be stored and used to cancel the event in the future.
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
            source: self.trigger.clone(),
            source_index: self.event_schedule_index,
            expire_handle: Some(handle),
            time,
            payload,
        };

        let new_event = Arc::new(new_event);
        let new_event_weak = Arc::downgrade(&new_event.clone());

        self.schedule.insert(new_event);
        self.expire_handles.insert(handle, new_event_weak);
        self.event_schedule_index += 1;

        Ok(handle)
    }

    /// This allows you to emit events from your transposer. the event is emitted at the current time.
    ///
    /// If you need to emit an event in the future at a scheduled time, schedule an internal event that can emit
    /// your event when handled.
    pub fn emit_event(&mut self, payload: T::Output) {
        let event = Event {
            timestamp: self.time(),
            payload,
        };
        self.output_events.push(event);
    }

    /// This allows you to expire an event currently in the schedule, as long as you have an [`ExpireHandle`].
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

    /// This allows you to exit the transposer, closing the output stream.
    pub fn exit(&mut self) {
        self.exit = true;
    }
}
