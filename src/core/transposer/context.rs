use futures::channel::oneshot::{channel, Receiver, Sender};

use super::{expire_handle::ExpireHandle, transposer_frame::TransposerFrameInternal, Transposer};

/// This is the interface through which you can do a variety of functions in your transposer.
///
/// the primary feature is scheduling events,
/// though there are more methods to interact with the engine.
pub struct InitContext<'a, T: Transposer> {
    // mutable references into the current transposer frame
    pub(super) frame_internal: TransposerFrameInternal<T>,

    // access to the input state
    input_state: &'a mut LazyState<T::InputState>,

    // values to output
    pub(super) outputs: Vec<T::Output>,
}

impl<'a, T: Transposer> InitContext<'a, T> {
    pub(super) fn new(input_state: &'a mut LazyState<T::InputState>) -> Self {
        Self {
            frame_internal: TransposerFrameInternal::new(),
            input_state,
            outputs: Vec::new(),
        }
    }

    pub async fn get_input_state(&mut self) -> Result<&T::InputState, &str> {
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
        self.outputs.push(payload);
    }
}

pub(super) enum LazyState<S> {
    Ready(S),
    Requested(Receiver<S>),
    Pending(Sender<Sender<S>>),
}

impl<S> LazyState<S> {
    pub async fn get(&mut self) -> &S {
        loop {
            match self {
                Self::Ready(s) => break s,
                Self::Requested(r) => {
                    break {
                        let s = r.await.unwrap();
                        std::mem::swap(self, &mut Self::Ready(s));
                        if let Self::Ready(s) = self {
                            s
                        } else {
                            unreachable!()
                        }
                    }
                }
                Self::Pending(_) => take_mut::take(self, |this| {
                    if let Self::Pending(s) = this {
                        let (sender, receiver) = channel();
                        let _ = s.send(sender);
                        Self::Requested(receiver)
                    } else {
                        unreachable!()
                    }
                }),
            }
        }
    }

    pub fn destroy(self) -> Option<S> {
        match self {
            Self::Ready(s) => Some(s),
            Self::Pending(_) => None,
            Self::Requested(mut r) => match r.try_recv() {
                Ok(s) => s,
                Err(_) => None,
            },
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

    // values to output
    pub(super) outputs: Vec<T::Output>,
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

            outputs: Vec::new(),
            exit: false,
        }
    }

    pub(super) fn new_scheduled(
        frame_internal: &'a mut TransposerFrameInternal<T>,
        input_state: &'a mut LazyState<T::InputState>,
    ) -> Self {
        Self {
            frame_internal,

            input_state,

            outputs: Vec::new(),
            exit: false,
        }
    }

    pub async fn get_input_state(&mut self) -> Result<&T::InputState, &str> {
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
        self.outputs.push(payload);
    }

    /// This allows you to expire an event currently in the schedule, as long as you have an [`ExpireHandle`].
    pub fn expire_event(&mut self, handle: ExpireHandle) -> Result<(T::Time, T::Scheduled), &str> {
        self.frame_internal.expire_event(handle)
    }

    /// This allows you to exit the transposer, closing the output stream.
    pub fn exit(&mut self) {
        self.exit = true;
    }
}

pub struct InterpolateContext<'a, T: Transposer> {
    input_state: &'a mut LazyState<T::InputState>,
}

impl<'a, T: Transposer> InterpolateContext<'a, T> {
    pub(super) fn new(
        input_state: &'a mut LazyState<T::InputState>,
    ) -> Self {
        Self {
            input_state,
        }
    }

    pub async fn get_input_state(&mut self) -> Result<&T::InputState, &str> {
        Ok(self.input_state.get().await)
    }
}
