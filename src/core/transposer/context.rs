use std::pin::Pin;

use futures::Future;

use super::{Transposer, expire_handle::ExpireHandle};



pub trait InitContext<'a, T: Transposer>: InputStateContext<'a, T> + ScheduleEventContext<T> + EmitEventContext<T> {}
impl<'a, U, T: Transposer> InitContext<'a, T> for U
where U: InputStateContext<'a, T> + ScheduleEventContext<T> + EmitEventContext<T> {}

pub trait InputContext<'a, T: Transposer>: InputStateContext<'a, T> + ScheduleEventContext<T> + ExpireEventContext<T> + EmitEventContext<T> + ExitContext {}
impl<'a, U, T: Transposer> InputContext<'a, T> for U
where U:  InputStateContext<'a, T> + ScheduleEventContext<T> + ExpireEventContext<T> + EmitEventContext<T> + ExitContext {}

pub trait ScheduleContext<'a, T: Transposer>: InputStateContext<'a, T> + ScheduleEventContext<T> + ExpireEventContext<T> + EmitEventContext<T> + ExitContext {}
impl<'a, U, T: Transposer> ScheduleContext<'a, T> for U
where U:  InputStateContext<'a, T> + ScheduleEventContext<T> + ExpireEventContext<T> + EmitEventContext<T> + ExitContext {}

pub trait InterpolateContext<'a, T: Transposer>: InputStateContext<'a, T> {}
impl<'a, U, T: Transposer> InterpolateContext<'a, T> for U
where U:InputStateContext<'a, T> {}

pub trait InputStateContext<'a, T: Transposer> {
    fn get_input_state<'f>(&'f mut self) -> Pin<&'f mut (dyn Future<Output=Result<&'a T::InputState, &'static str>>)>;
}

pub trait ScheduleEventContext<T: Transposer> {
    fn schedule_event(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<(), &str>;

    fn schedule_event_expireable(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<ExpireHandle, &str>;
}

pub trait ExpireEventContext<T: Transposer> {
    fn expire_event(&mut self, handle: ExpireHandle) -> Result<(T::Time, T::Scheduled), &str>;
}

pub trait EmitEventContext<T: Transposer> {
    fn emit_event(&mut self, payload: T::Output);
}

pub trait ExitContext {
    fn exit(&mut self);
}