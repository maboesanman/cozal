use core::future::Future;
use core::pin::Pin;

use super::super::frame::TransposerFrameInternal;
use super::lazy_state::LazyState;
use crate::source::adapters::transpose::engine_time::{EngineTime, EngineTimeSchedule};
use crate::transposer::context::*;
use crate::transposer::{ExpireHandle, Transposer};

/// This is the interface through which you can do a variety of functions in your transposer.
///
/// the primary features are scheduling and expiring events,
/// though there are more methods to interact with the engine.
pub struct UpdateContext<T: Transposer>
where
    T::Scheduled: Clone,
{
    // these are pointers because this is stored next to the targets.
    frame_internal: *mut TransposerFrameInternal<T>,
    input_state:    *mut LazyState<T::InputState>,

    time:           EngineTime<T::Time>,
    schedule_index: usize,

    // values to output
    outputs: Vec<T::Output>,
}

impl<T: Transposer> InitContext<T> for UpdateContext<T> {}
impl<T: Transposer> HandleInputContext<T> for UpdateContext<T> {}
impl<T: Transposer> HandleScheduleContext<T> for UpdateContext<T> {}

impl<T: Transposer> UpdateContext<T> {
    pub(super) unsafe fn new(
        time: EngineTime<T::Time>,
        frame_internal: *mut TransposerFrameInternal<T>,
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

    pub(super) fn recover_outputs(self) -> Vec<T::Output> {
        self.outputs
    }

    fn get_frame_internal_mut(&mut self) -> &mut TransposerFrameInternal<T> {
        unsafe { self.frame_internal.as_mut().unwrap() }
    }

    fn get_input_state_mut(&mut self) -> &mut LazyState<T::InputState> {
        unsafe { self.input_state.as_mut().unwrap() }
    }
}

impl<T: Transposer> InputStateContext<T> for UpdateContext<T> {
    fn get_input_state(&mut self) -> Pin<&mut dyn Future<Output = T::InputState>> {
        unsafe { Pin::new_unchecked(self.get_input_state_mut()) }
    }
}

impl<T: Transposer> ScheduleEventContext<T> for UpdateContext<T> {
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

impl<T: Transposer> ExpireEventContext<T> for UpdateContext<T> {
    fn expire_event(
        &mut self,
        handle: ExpireHandle,
    ) -> Result<(T::Time, T::Scheduled), ExpireEventError> {
        self.get_frame_internal_mut().expire_event(handle)
    }
}

impl<T: Transposer> EmitEventContext<T> for UpdateContext<T> {
    fn emit_event(&mut self, payload: T::Output) {
        self.outputs.push(payload);
    }
}

impl<T: Transposer> RngContext for UpdateContext<T> {
    fn get_rng(&mut self) -> &mut dyn rand::RngCore {
        &mut self.get_frame_internal_mut().rng
    }
}

// impl<'a, T: Transposer> InitContext<'a, T> for ReUpdateContext<'a, T> {}
// impl<'a, T: Transposer> HandleInputContext<'a, T> for ReUpdateContext<'a, T> {}
// impl<'a, T: Transposer> HandleScheduleContext<'a, T> for ReUpdateContext<'a, T> {}

// pub struct ReUpdateContext<'a, T: Transposer>
// where
//     T::Scheduled: Clone,
// {
//     // mutable references into the current transposer frame
//     frame_internal: &'a mut TransposerFrameInternal<T>,

//     // access to the input state
//     input_state: &'a mut LazyState<T::InputState>,
// }

// impl<'a, T: Transposer> InputStateContext<'a, T> for ReUpdateContext<'a, T> {
//     fn get_input_state(&mut self) -> Pin<&mut dyn Future<Output = T::InputState>> {
//         Pin::<&mut &mut LazyState<T::InputState>>::new(&mut self.input_state)
//     }
// }

// impl<'a, T: Transposer> ScheduleEventContext<T> for ReUpdateContext<'a, T> {
//     fn schedule_event(
//         &mut self,
//         time: T::Time,
//         payload: T::Scheduled,
//     ) -> Result<(), ScheduleEventError> {
//         self.frame_internal.schedule_event(time, payload)
//     }

//     fn schedule_event_expireable(
//         &mut self,
//         time: T::Time,
//         payload: T::Scheduled,
//     ) -> Result<ExpireHandle, ScheduleEventError> {
//         self.frame_internal.schedule_event_expireable(time, payload)
//     }
// }

// impl<'a, T: Transposer> ExpireEventContext<T> for ReUpdateContext<'a, T> {
//     fn expire_event(
//         &mut self,
//         handle: ExpireHandle,
//     ) -> Result<(T::Time, T::Scheduled), ExpireEventError> {
//         self.frame_internal.expire_event(handle)
//     }
// }

// impl<'a, T: Transposer> EmitEventContext<T> for ReUpdateContext<'a, T> {
//     fn emit_event(&mut self, _payload: T::Output) {}
// }

// impl<'a, T: Transposer> RngContext for ReUpdateContext<'a, T> {
//     fn get_rng(&mut self) -> &mut dyn rand::RngCore {
//         &mut self.frame_internal.rng
//     }
// }
