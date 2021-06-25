use core::pin::Pin;
use core::future::Future;

use super::super::{context::*, ExpireHandle, Transposer};

use super::{lazy_state::LazyState, transposer_frame::TransposerFrameInternal};

/// This is the interface through which you can do a variety of functions in your transposer.
///
/// the primary features are scheduling and expiring events,
/// though there are more methods to interact with the engine.
pub struct EngineContext<'a, T: Transposer>
where
    T::Scheduled: Clone,
{
    // mutable references into the current transposer frame
    frame_internal: &'a mut TransposerFrameInternal<'a, T>,

    // access to the input state
    input_state: &'a mut LazyState<T::InputState>,

    // values to output
    pub(super) outputs: Vec<T::Output>,
}

impl<'a, T: Transposer> InitContext<'a, T> for EngineContext<'a, T> {}
impl<'a, T: Transposer> HandleInputContext<'a, T> for EngineContext<'a, T> {}
impl<'a, T: Transposer> HandleScheduleContext<'a, T> for EngineContext<'a, T> {}

impl<'a, T: Transposer> EngineContext<'a, T> {
    pub(super) fn new(
        frame_internal: &'a mut TransposerFrameInternal<'a, T>,
        input_state: &'a mut LazyState<T::InputState>,
    ) -> Self {
        Self {
            frame_internal,
            input_state,
            outputs: Vec::new(),
        }
    }
}

impl<'a, T: Transposer> InputStateContext<'a, T> for EngineContext<'a, T> {
    fn get_input_state(&mut self) -> Pin<&mut dyn Future<Output = T::InputState>> {
        Pin::<&mut &mut LazyState<T::InputState>>::new(&mut self.input_state)
    }
}

impl<'a, T: Transposer> ScheduleEventContext<T> for EngineContext<'a, T> {
    fn schedule_event(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<(), ScheduleEventError> {
        self.frame_internal.schedule_event(time, payload)
    }

    fn schedule_event_expireable(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<ExpireHandle, ScheduleEventError> {
        self.frame_internal.schedule_event_expireable(time, payload)
    }
}

impl<'a, T: Transposer> ExpireEventContext<T> for EngineContext<'a, T> {
    fn expire_event(
        &mut self,
        handle: ExpireHandle,
    ) -> Result<(T::Time, T::Scheduled), ExpireEventError> {
        self.frame_internal.expire_event(handle)
    }
}

impl<'a, T: Transposer> EmitEventContext<T> for EngineContext<'a, T> {
    fn emit_event(&mut self, payload: T::Output) {
        self.outputs.push(payload);
    }
}

impl<'a, T: Transposer> RngContext for EngineContext<'a, T> {
    fn get_rng(&mut self) -> &mut dyn rand::RngCore {
        &mut self.frame_internal.rng
    }
}

pub struct EngineRebuildContext<'a, T: Transposer>
where
    T::Scheduled: Clone,
{
    // mutable references into the current transposer frame
    frame_internal: &'a mut TransposerFrameInternal<'a, T>,

    // access to the input state
    input_state: &'a mut LazyState<T::InputState>,
}

impl<'a, T: Transposer> InputStateContext<'a, T> for EngineRebuildContext<'a, T> {
    fn get_input_state(&mut self) -> Pin<&mut dyn Future<Output = T::InputState>> {
        Pin::<&mut &mut LazyState<T::InputState>>::new(&mut self.input_state)
    }
}

impl<'a, T: Transposer> ScheduleEventContext<T> for EngineRebuildContext<'a, T> {
    fn schedule_event(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<(), ScheduleEventError> {
        self.frame_internal.schedule_event(time, payload)
    }

    fn schedule_event_expireable(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<ExpireHandle, ScheduleEventError> {
        self.frame_internal.schedule_event_expireable(time, payload)
    }
}

impl<'a, T: Transposer> ExpireEventContext<T> for EngineRebuildContext<'a, T> {
    fn expire_event(
        &mut self,
        handle: ExpireHandle,
    ) -> Result<(T::Time, T::Scheduled), ExpireEventError> {
        self.frame_internal.expire_event(handle)
    }
}

impl<'a, T: Transposer> EmitEventContext<T> for EngineRebuildContext<'a, T> {
    fn emit_event(&mut self, _payload: T::Output) {}
}
