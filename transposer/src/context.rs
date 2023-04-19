use core::future::Future;
use core::pin::Pin;
use std::ptr::NonNull;

use rand::RngCore;

use super::expire_handle::ExpireHandle;
use super::Transposer;
use crate::{StateRetriever, TransposerInput};

pub trait InitContext<'a, T: Transposer>:
    CurrentTimeContext<T>
    + InputStateContext<'a, T>
    + ScheduleEventContext<T>
    + EmitEventContext<T>
    + RngContext
{
}

pub trait HandleInputContext<'a, T: Transposer>:
    CurrentTimeContext<T>
    + LastUpdatedTimeContext<T>
    + InputStateContext<'a, T>
    + ScheduleEventContext<T>
    + ExpireEventContext<T>
    + EmitEventContext<T>
    + RngContext
{
}

pub trait HandleScheduleContext<'a, T: Transposer>:
    CurrentTimeContext<T>
    + LastUpdatedTimeContext<T>
    + InputStateContext<'a, T>
    + ScheduleEventContext<T>
    + ExpireEventContext<T>
    + EmitEventContext<T>
    + RngContext
{
}

pub trait InterpolateContext<'a, T: Transposer>:
    CurrentTimeContext<T> + LastUpdatedTimeContext<T> + InputStateContext<'a, T>
{
}

pub trait CurrentTimeContext<T: Transposer> {
    fn current_time(&self) -> T::Time;
}

pub trait LastUpdatedTimeContext<T: Transposer> {
    fn last_updated_time(&self) -> T::Time;
}

pub trait InputStateContext<'a, T: Transposer> {
    #[doc(hidden)]
    fn get_input_state_manager(&mut self) -> &'a T::InputStateManager;
}

pub trait InputStateContextExt<'a, T: Transposer>: InputStateContext<'a, T> {
    async fn get_input_state<I: TransposerInput<Base = T>>(&mut self) -> &'a I::InputState
    where
        T::InputStateManager: 'a + StateRetriever<I>;
}

impl<'a, T: Transposer, C: InputStateContext<'a, T> + ?Sized> InputStateContextExt<'a, T> for C {
    async fn get_input_state<I: TransposerInput<Base = T>>(&mut self) -> &'a I::InputState
    where
        T::InputStateManager: 'a + StateRetriever<I>,
    {
        let ptr: NonNull<_> = self
            .get_input_state_manager()
            .get_input_state()
            .await
            .unwrap();

        unsafe { ptr.as_ref() }
    }
}

pub trait ScheduleEventContext<T: Transposer> {
    #[must_use]
    fn schedule_event(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<(), ScheduleEventError>;

    #[must_use]
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
    #[must_use]
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
    #[must_use]
    fn emit_event(&mut self, payload: T::OutputEvent) -> Pin<Box<dyn '_ + Future<Output = ()>>>;
}

pub trait RngContext {
    #[must_use]
    fn get_rng(&mut self) -> &mut dyn RngCore;
}

trait TransposerInputStateProvider<I: TransposerInput + ?Sized> {
    #[must_use]
    async fn get_input_state<'a>(&'a self) -> &'a I::InputState
    where
        I::InputState: 'a;
}
