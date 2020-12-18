use crate::core::Event;
use futures::channel::oneshot::{Receiver, Sender};

use super::{
    expire_handle::ExpireHandle, transposer_frame::TransposerFrameInternal, InternalOutputEvent,
    Transposer,
};

/// This is the interface through which you can do a variety of functions in your transposer.
///
/// the primary feature is scheduling events,
/// though there are more methods to interact with the engine.
pub struct InitContext<T: Transposer> {
    // mutable references into the current transposer frame
    pub(super) frame_internal: TransposerFrameInternal<T>,

    // values to output
    pub(super) output_events: Vec<InternalOutputEvent<T>>,
}

impl<T: Transposer> InitContext<T> {
    pub(super) fn new() -> Self {
        Self {
            frame_internal: TransposerFrameInternal::new(),
            output_events: Vec::new(),
        }
    }

    fn time(&self) -> T::Time {
        self.frame_internal.time()
    }

    /// This allows you to schedule events to happen in the future.
    /// As long as the time you supply is not less than the current time,
    /// the event can be scheduled.
    pub fn schedule_event(&mut self, time: T::Time, payload: T::Scheduled) -> Result<(), &str> {
        self.frame_internal.schedule_event(time, payload)
    }

    /// The same behavior as [`schedule_event`], but now returning an [`ExpireHandle`]
    /// which can be stored and used to cancel the event in the future.
    pub fn schedule_event_expireable(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<ExpireHandle, &str> {
        self.frame_internal.schedule_event_expireable(time, payload)
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
    // mutable references into the current transposer frame
    frame_internal: &'a mut TransposerFrameInternal<T>,

    // access to the input state
    input_state: &'a mut LazyState<T::InputState>,
    pub(super) input_state_requester: Option<Sender<()>>,

    // values to output
    pub(super) output_events: Vec<InternalOutputEvent<T>>,
    pub(super) exit: bool,
}

impl<'a, T: Transposer> UpdateContext<'a, T> {
    pub(super) fn new_input(
        // this can't be a mutable reference to the frame because the borrow needs to be split.
        frame_internal: &'a mut TransposerFrameInternal<T>,
        input_state: &'a mut LazyState<T::InputState>,
    ) -> Self {
        Self {
            frame_internal,

            input_state,
            input_state_requester: None,

            output_events: Vec::new(),
            exit: false,
        }
    }

    pub(super) fn new_scheduled(
        frame_internal: &'a mut TransposerFrameInternal<T>,
        input_state: &'a mut LazyState<T::InputState>,
        input_state_requester: Option<Sender<()>>,
    ) -> Self {
        Self {
            frame_internal,

            input_state,
            input_state_requester,

            output_events: Vec::new(),
            exit: false,
        }
    }

    fn time(&self) -> T::Time {
        self.frame_internal.time()
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
        self.frame_internal.schedule_event(time, payload)
    }

    /// The same behavior as [`schedule_event`], but now returning an [`ExpireHandle`]
    /// which can be stored and used to cancel the event in the future.
    pub fn schedule_event_expireable(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<ExpireHandle, &str> {
        self.frame_internal.schedule_event_expireable(time, payload)
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
        self.frame_internal.expire_event(handle)
    }

    /// This allows you to exit the transposer, closing the output stream.
    pub fn exit(&mut self) {
        self.exit = true;
    }
}
