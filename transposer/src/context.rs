use core::future::Future;
use core::pin::Pin;

use rand::RngCore;

use super::expire_handle::ExpireHandle;
use super::Transposer;
use crate::{TransposerInput, StateRetriever};

pub trait InitContext<'a, T: Transposer>:
    InputStateContext<'a, T> + ScheduleEventContext<T> + EmitEventContext<T> + RngContext
{
}

pub trait HandleInputContext<'a, T: Transposer>:
    InputStateContext<'a, T>
    + ScheduleEventContext<T>
    + ExpireEventContext<T>
    + EmitEventContext<T>
    + RngContext
{
}

pub trait HandleScheduleContext<'a, T: Transposer>:
    InputStateContext<'a, T>
    + ScheduleEventContext<T>
    + ExpireEventContext<T>
    + EmitEventContext<T>
    + RngContext
{
}

pub trait InterpolateContext<'a, T: Transposer>: InputStateContext<'a, T> {}

pub trait InputStateContext<'a, T: Transposer> {
    #[doc(hidden)]
    fn get_input_state_manager(&mut self) -> &'a T::InputStateManager;
}

pub trait InputStateContextExt<'a, T: Transposer>: InputStateContext<'a, T> {
    async fn get_input_state<I: TransposerInput<Base=T>>(&mut self) -> &'a I::InputState
    where T::InputStateManager: 'a + StateRetriever<T, I>;
}

impl<'a, T: Transposer, C: InputStateContext<'a, T> + ?Sized> InputStateContextExt<'a, T> for C {
    async fn get_input_state<I: TransposerInput<Base=T>>(&mut self) -> &'a I::InputState
    where T::InputStateManager: 'a + StateRetriever<T, I>
    {
        self.get_input_state_manager().get_input_state().await.unwrap()
    }
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

trait TransposerInputStateProvider<I: TransposerInput + ?Sized>
{
    async fn get_input_state<'a>(&'a self) -> &'a I::InputState
    where I::InputState: 'a;
}
