use core::future::Future;
use core::pin::Pin;

use rand::RngCore;

use super::expire_handle::ExpireHandle;
use super::Transposer;
use crate::TransposerInput;

pub trait InitContext<'a, T: Transposer>:
    InputStateContext<T> + ScheduleEventContext<T> + EmitEventContext<T> + RngContext
{
}

pub trait HandleInputContext<'a, T: Transposer>:
    InputStateContext<T>
    + ScheduleEventContext<T>
    + ExpireEventContext<T>
    + EmitEventContext<T>
    + RngContext
{
}

pub trait HandleScheduleContext<'a, T: Transposer>:
    InputStateContext<T>
    + ScheduleEventContext<T>
    + ExpireEventContext<T>
    + EmitEventContext<T>
    + RngContext
{
}

pub trait InterpolateContext<'a, T: Transposer>: InputStateContext<T> {}

pub trait InputStateContext<T: Transposer> {
    fn get_input_state_requester(&mut self) -> &mut T::InputStateProvider;
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

#[non_exhaustive]
#[derive(Debug)]
pub enum ScheduleEventError {
    NewEventBeforeCurrent,
}

pub trait ExpireEventContext<T: Transposer> {
    fn expire_event(
        &mut self,
        handle: ExpireHandle,
    ) -> Result<(T::Time, T::Scheduled), ExpireEventError>;
}

#[non_exhaustive]
#[derive(Debug)]
pub enum ExpireEventError {
    InvalidOrUsedHandle,
}

pub trait EmitEventContext<T: Transposer> {
    fn emit_event(&mut self, payload: T::OutputEvent) -> Pin<Box<dyn '_ + Future<Output = ()>>>;
}

pub trait RngContext {
    fn get_rng(&mut self) -> &mut dyn RngCore;
}

trait TransposerInputStateProvider<'t, I: TransposerInput + ?Sized>
where
    I::InputState: 't,
{
    async fn get_input_state(&mut self) -> &'t I::InputState;
}
