use std::pin::Pin;

use futures::Future;

use super::{expire_handle::ExpireHandle, Transposer};

pub trait InitContext<'a, T: Transposer>:
    InputStateContext<'a, T> + ScheduleEventContext<T> + EmitEventContext<T>
{
}
impl<'a, U, T: Transposer> InitContext<'a, T> for U where
    U: InputStateContext<'a, T> + ScheduleEventContext<T> + EmitEventContext<T>
{
}

pub trait InputContext<'a, T: Transposer>:
    InputStateContext<'a, T> + ScheduleEventContext<T> + ExpireEventContext<T> + EmitEventContext<T>
{
}
impl<'a, U, T: Transposer> InputContext<'a, T> for U where
    U: InputStateContext<'a, T>
        + ScheduleEventContext<T>
        + ExpireEventContext<T>
        + EmitEventContext<T>
{
}

pub trait ScheduleContext<'a, T: Transposer>:
    InputStateContext<'a, T> + ScheduleEventContext<T> + ExpireEventContext<T> + EmitEventContext<T>
{
}
impl<'a, U, T: Transposer> ScheduleContext<'a, T> for U where
    U: InputStateContext<'a, T>
        + ScheduleEventContext<T>
        + ExpireEventContext<T>
        + EmitEventContext<T>
{
}

pub trait InputStateContext<'a, T: Transposer> {
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
