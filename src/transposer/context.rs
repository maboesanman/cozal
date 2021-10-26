use core::future::Future;
use core::pin::Pin;

use rand::RngCore;

use super::expire_handle::ExpireHandle;
use super::Transposer;

pub trait InitContext<T: Transposer>:
    InputStateContext<T> + ScheduleEventContext<T> + EmitEventContext<T> + RngContext
{
}

pub trait HandleInputContext<T: Transposer>:
    InputStateContext<T>
    + ScheduleEventContext<T>
    + ExpireEventContext<T>
    + EmitEventContext<T>
    + RngContext
{
}

pub trait HandleScheduleContext<T: Transposer>:
    InputStateContext<T>
    + ScheduleEventContext<T>
    + ExpireEventContext<T>
    + EmitEventContext<T>
    + RngContext
{
}

pub trait InterpolateContext<T: Transposer>: InputStateContext<T> {}

pub trait InputStateContext<T: Transposer> {
    fn get_input_state(&mut self) -> Pin<&mut dyn Future<Output = T::InputState>>;
}

pub trait ScheduleEventContext<T: Transposer> {
    fn schedule_event(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<(), ScheduleEventError>;

    fn schedule_event_expireable(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<ExpireHandle, ScheduleEventError>;
}

pub enum ScheduleEventError {
    NewEventBeforeCurrent,
}

pub trait ExpireEventContext<T: Transposer> {
    fn expire_event(
        &mut self,
        handle: ExpireHandle,
    ) -> Result<(T::Time, T::Scheduled), ExpireEventError>;
}
pub enum ExpireEventError {
    ExpiredEvent,
    InvalidHandle,
}

pub trait EmitEventContext<T: Transposer> {
    fn emit_event(&mut self, payload: T::Output);
}

pub trait RngContext {
    fn get_rng(&mut self) -> &mut dyn RngCore;
}
