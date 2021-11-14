use core::future::Future;
use core::pin::Pin;

use super::super::frame::FrameMetaData;
use super::engine_time::{EngineTime, EngineTimeSchedule};
use super::lazy_state::LazyState;
use crate::transposer::context::*;
use crate::transposer::{ExpireHandle, Transposer};

pub trait UpdateContext<T: Transposer>:
    InitContext<T> + HandleInputContext<T> + HandleScheduleContext<T>
where
    T::Scheduled: Clone,
{
    type Outputs;

    // SAFETY: ensure this UpdateContext is dropped before frame_internal and input_state.
    unsafe fn new(
        time: EngineTime<T::Time>,
        frame_internal: *mut FrameMetaData<T>,
        input_state: *mut LazyState<T::InputState>,
    ) -> Self;

    fn recover_outputs(self) -> Self::Outputs;
}

/// This is the interface through which you can do a variety of functions in your transposer.
///
/// the primary features are scheduling and expiring events,
/// though there are more methods to interact with the engine.
pub struct OriginalUpdateContext<T: Transposer>
where
    T::Scheduled: Clone,
{
    // these are pointers because this is stored next to the targets.
    frame_internal: *mut FrameMetaData<T>,
    input_state:    *mut LazyState<T::InputState>,

    time:           EngineTime<T::Time>,
    schedule_index: usize,

    // values to output
    outputs: Vec<T::Output>,
}

impl<T: Transposer> InitContext<T> for OriginalUpdateContext<T> {}
impl<T: Transposer> HandleInputContext<T> for OriginalUpdateContext<T> {}
impl<T: Transposer> HandleScheduleContext<T> for OriginalUpdateContext<T> {}
impl<T: Transposer> UpdateContext<T> for OriginalUpdateContext<T> {
    type Outputs = Vec<T::Output>;

    unsafe fn new(
        time: EngineTime<T::Time>,
        frame_internal: *mut FrameMetaData<T>,
        input_state: *mut LazyState<T::InputState>,
    ) -> Self {
        Self {
            frame_internal,
            input_state,
            time,
            schedule_index: 0,
            outputs: Vec::new(),
        }
    }

    fn recover_outputs(self) -> Self::Outputs {
        self.outputs
    }
}

impl<T: Transposer> OriginalUpdateContext<T> {
    // SAFETY: ensure this UpdateContext is dropped before frame_internal and input_state.

    fn get_frame_internal_mut(&mut self) -> &mut FrameMetaData<T> {
        unsafe { self.frame_internal.as_mut().unwrap() }
    }

    fn get_input_state_mut(&mut self) -> &mut LazyState<T::InputState> {
        unsafe { self.input_state.as_mut().unwrap() }
    }
}

impl<T: Transposer> InputStateContext<T> for OriginalUpdateContext<T> {
    fn get_input_state(&mut self) -> Pin<&mut dyn Future<Output = T::InputState>> {
        unsafe { Pin::new_unchecked(self.get_input_state_mut()) }
    }
}

impl<T: Transposer> ScheduleEventContext<T> for OriginalUpdateContext<T> {
    fn schedule_event(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<(), ScheduleEventError> {
        if time < self.time.raw_time().unwrap() {
            return Err(ScheduleEventError::NewEventBeforeCurrent)
        }

        let schedule_time = EngineTimeSchedule {
            time,
            parent: self.time.clone(),
            parent_index: self.schedule_index,
        };
        self.get_frame_internal_mut()
            .schedule_event(schedule_time, payload);
        self.schedule_index += 1;

        Ok(())
    }

    fn schedule_event_expireable(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<ExpireHandle, ScheduleEventError> {
        if time < self.time.raw_time().unwrap() {
            return Err(ScheduleEventError::NewEventBeforeCurrent)
        }

        let time = EngineTimeSchedule {
            time,
            parent: self.time.clone(),
            parent_index: self.schedule_index,
        };

        let handle = self
            .get_frame_internal_mut()
            .schedule_event_expireable(time, payload);
        self.schedule_index += 1;

        Ok(handle)
    }
}

impl<T: Transposer> ExpireEventContext<T> for OriginalUpdateContext<T> {
    fn expire_event(
        &mut self,
        handle: ExpireHandle,
    ) -> Result<(T::Time, T::Scheduled), ExpireEventError> {
        self.get_frame_internal_mut().expire_event(handle)
    }
}

impl<T: Transposer> EmitEventContext<T> for OriginalUpdateContext<T> {
    fn emit_event(&mut self, payload: T::Output) {
        self.outputs.push(payload);
    }
}

impl<T: Transposer> RngContext for OriginalUpdateContext<T> {
    fn get_rng(&mut self) -> &mut dyn rand::RngCore {
        &mut self.get_frame_internal_mut().rng
    }
}

pub struct RepeatUpdateContext<T: Transposer>
where
    T::Scheduled: Clone,
{
    // these are pointers because this is stored next to the targets.
    frame_internal: *mut FrameMetaData<T>,
    input_state:    *mut LazyState<T::InputState>,

    time:           EngineTime<T::Time>,
    schedule_index: usize,
}

impl<T: Transposer> InitContext<T> for RepeatUpdateContext<T> {}
impl<T: Transposer> HandleInputContext<T> for RepeatUpdateContext<T> {}
impl<T: Transposer> HandleScheduleContext<T> for RepeatUpdateContext<T> {}
impl<T: Transposer> UpdateContext<T> for RepeatUpdateContext<T> {
    type Outputs = ();

    unsafe fn new(
        time: EngineTime<T::Time>,
        frame_internal: *mut FrameMetaData<T>,
        input_state: *mut LazyState<T::InputState>,
    ) -> Self {
        Self {
            frame_internal,
            input_state,
            time,
            schedule_index: 0,
        }
    }

    fn recover_outputs(self) -> Self::Outputs {}
}

impl<T: Transposer> RepeatUpdateContext<T> {
    // SAFETY: ensure this UpdateContext is dropped before frame_internal and input_state.

    fn get_frame_internal_mut(&mut self) -> &mut FrameMetaData<T> {
        unsafe { self.frame_internal.as_mut().unwrap() }
    }

    fn get_input_state_mut(&mut self) -> &mut LazyState<T::InputState> {
        unsafe { self.input_state.as_mut().unwrap() }
    }
}

impl<T: Transposer> InputStateContext<T> for RepeatUpdateContext<T> {
    fn get_input_state(&mut self) -> Pin<&mut dyn Future<Output = T::InputState>> {
        unsafe { Pin::new_unchecked(self.get_input_state_mut()) }
    }
}

impl<T: Transposer> ScheduleEventContext<T> for RepeatUpdateContext<T> {
    fn schedule_event(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<(), ScheduleEventError> {
        if time < self.time.raw_time().unwrap() {
            return Err(ScheduleEventError::NewEventBeforeCurrent)
        }

        let schedule_time = EngineTimeSchedule {
            time,
            parent: self.time.clone(),
            parent_index: self.schedule_index,
        };
        self.get_frame_internal_mut()
            .schedule_event(schedule_time, payload);
        self.schedule_index += 1;

        Ok(())
    }

    fn schedule_event_expireable(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<ExpireHandle, ScheduleEventError> {
        if time < self.time.raw_time().unwrap() {
            return Err(ScheduleEventError::NewEventBeforeCurrent)
        }

        let time = EngineTimeSchedule {
            time,
            parent: self.time.clone(),
            parent_index: self.schedule_index,
        };

        let handle = self
            .get_frame_internal_mut()
            .schedule_event_expireable(time, payload);
        self.schedule_index += 1;

        Ok(handle)
    }
}

impl<T: Transposer> ExpireEventContext<T> for RepeatUpdateContext<T> {
    fn expire_event(
        &mut self,
        handle: ExpireHandle,
    ) -> Result<(T::Time, T::Scheduled), ExpireEventError> {
        self.get_frame_internal_mut().expire_event(handle)
    }
}

impl<T: Transposer> EmitEventContext<T> for RepeatUpdateContext<T> {
    fn emit_event(&mut self, _payload: T::Output) {}
}

impl<T: Transposer> RngContext for RepeatUpdateContext<T> {
    fn get_rng(&mut self) -> &mut dyn rand::RngCore {
        &mut self.get_frame_internal_mut().rng
    }
}
