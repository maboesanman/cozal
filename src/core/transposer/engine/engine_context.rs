use std::pin::Pin;

use futures::{Future, channel::oneshot::{channel, Receiver, Sender}};

use crate::core::{Transposer, transposer::{context::{EmitEventContext, ExitContext, ExpireEventContext, InputStateContext, ScheduleEventContext}, expire_handle::ExpireHandle}};

use super::transposer_frame::TransposerFrameInternal;


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
pub struct EngineContext<'a, T: Transposer> 
where T::Scheduled: Clone {
    // mutable references into the current transposer frame
    frame_internal: &'a mut TransposerFrameInternal<T>,

    // access to the input state
    input_state: &'a mut LazyState<T::InputState>,

    // values to output
    pub(super) outputs: Vec<T::Output>,
    pub(super) exit: bool,
}

impl<'a, T: Transposer> EngineContext<'a, T> {
    pub(super) fn new(frame_internal: &'a mut TransposerFrameInternal<T>, input_state: &'a mut LazyState<T::InputState>) -> Self {
        Self {
            frame_internal,
            input_state,
            outputs: Vec::new(),
            exit: false,
        }
    }
}

// this is gonna be tricky...
impl<'a, T: Transposer> InputStateContext<'a, T> for EngineContext<'a, T> {
    fn get_input_state<'f>(&'f mut self) -> Pin<&'f mut (dyn Future<Output=Result<&'a T::InputState, &'static str>>)> {
        unimplemented!()
    }
}

impl<'a, T: Transposer> ScheduleEventContext<T> for EngineContext<'a, T> {
    fn schedule_event(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<(), &str> {
        self.frame_internal.schedule_event(time, payload)
    }

    fn schedule_event_expireable(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<ExpireHandle, &str> {
        self.frame_internal.schedule_event_expireable(time, payload)
    }
}

impl<'a, T: Transposer> ExpireEventContext<T> for EngineContext<'a, T> {
    fn expire_event(&mut self, handle: ExpireHandle) -> Result<(T::Time, T::Scheduled), &str> {
        self.frame_internal.expire_event(handle)
    }
}

impl<'a, T: Transposer> EmitEventContext<T> for EngineContext<'a, T> {
    fn emit_event(&mut self, payload: T::Output) {
        self.outputs.push(payload);
    }
}

impl<'a, T: Transposer> ExitContext for EngineContext<'a, T> {
    fn exit(&mut self) {
        self.exit = true;
    }
}

pub struct EngineRebuildContext<'a, T: Transposer> 
where T::Scheduled: Clone {
    // mutable references into the current transposer frame
    frame_internal: &'a mut TransposerFrameInternal<T>,

    // access to the input state
    input_state: &'a mut LazyState<T::InputState>,
}

// this is gonna be tricky...
impl<'a, T: Transposer> InputStateContext<'a, T> for EngineRebuildContext<'a, T> {
    fn get_input_state<'f>(&'f mut self) -> Pin<&'f mut (dyn Future<Output=Result<&'a T::InputState, &'static str>>)> {
        unimplemented!()
    }
}

impl<'a, T: Transposer> ScheduleEventContext<T> for EngineRebuildContext<'a, T> {
    fn schedule_event(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<(), &str> {
        self.frame_internal.schedule_event(time, payload)
    }

    fn schedule_event_expireable(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<ExpireHandle, &str> {
        self.frame_internal.schedule_event_expireable(time, payload)
    }
}

impl<'a, T: Transposer> ExpireEventContext<T> for EngineRebuildContext<'a, T> {
    fn expire_event(&mut self, handle: ExpireHandle) -> Result<(T::Time, T::Scheduled), &str> {
        self.frame_internal.expire_event(handle)
    }
}

impl<'a, T: Transposer> EmitEventContext<T> for EngineRebuildContext<'a, T> {
    fn emit_event(&mut self, _payload: T::Output) { }
}

impl<'a, T: Transposer> ExitContext for EngineRebuildContext<'a, T> {
    fn exit(&mut self) { }
}

pub struct EngineInterpolationContext<'a, T: Transposer> 
where T::Scheduled: Clone {
    // access to the input state
    input_state: &'a mut LazyState<T::InputState>,
}

impl<'a, T: Transposer> EngineInterpolationContext<'a, T> {
    pub(super) fn new(input_state: &'a mut LazyState<T::InputState>) -> Self {
        Self {
            input_state,
        }
    }
}

// this is gonna be tricky...
impl<'a, T: Transposer> InputStateContext<'a, T> for EngineInterpolationContext<'a, T> {
    fn get_input_state<'f>(&'f mut self) -> Pin<&'f mut (dyn Future<Output=Result<&'a T::InputState, &'static str>>)> {
        unimplemented!()
    }
}