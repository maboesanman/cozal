use core::future::Future;
use core::pin::Pin;

use super::time::StepTime;
use super::update::{TransposerMetaData, UpdateContext};
use crate::transposer::context::*;
use crate::transposer::schedule_storage::StorageFamily;
use crate::transposer::step_group::lazy_state::LazyState;
use crate::transposer::{ExpireHandle, Transposer};

pub trait OutputCollector<O> {
    fn new() -> Self;
    fn push(&mut self, item: O);
}

impl<O> OutputCollector<O> for Vec<O> {
    fn new() -> Self {
        Vec::new()
    }
    fn push(&mut self, item: O) {
        self.push(item)
    }
}

impl<O> OutputCollector<O> for () {
    fn new() -> Self {}
    fn push(&mut self, _item: O) {}
}

/// This is the interface through which you can do a variety of functions in your transposer.
///
/// the primary features are scheduling and expiring events,
/// though there are more methods to interact with the engine.
pub struct StepUpdateContext<T: Transposer, S: StorageFamily, C: OutputCollector<T::Output>> {
    // these are pointers because this is stored next to the targets.
    frame_internal: *mut TransposerMetaData<T, S>,
    input_state:    *const LazyState<T::InputState>,

    time:                   StepTime<T::Time>,
    current_emission_index: usize,

    // values to output
    output_collector: C,
}

impl<'a, T: Transposer, S: StorageFamily, C: OutputCollector<T::Output>> InitContext<'a, T>
    for StepUpdateContext<T, S, C>
{
}
impl<'a, T: Transposer, S: StorageFamily, C: OutputCollector<T::Output>> HandleInputContext<'a, T>
    for StepUpdateContext<T, S, C>
{
}
impl<'a, T: Transposer, S: StorageFamily, C: OutputCollector<T::Output>>
    HandleScheduleContext<'a, T> for StepUpdateContext<T, S, C>
{
}
impl<T: Transposer, S: StorageFamily, C: OutputCollector<T::Output>> UpdateContext<T, S>
    for StepUpdateContext<T, S, C>
{
    type Outputs = C;

    // SAFETY: need to gurantee the pointers outlive this object.
    unsafe fn new(
        time: StepTime<T::Time>,
        frame_internal: *mut TransposerMetaData<T, S>,
        input_state: *const LazyState<T::InputState>,
    ) -> Self {
        Self {
            frame_internal,
            input_state,
            time,
            current_emission_index: 0,
            output_collector: C::new(),
        }
    }

    fn recover_outputs(self) -> Self::Outputs {
        self.output_collector
    }
}

impl<T: Transposer, S: StorageFamily, C: OutputCollector<T::Output>> StepUpdateContext<T, S, C> {
    fn get_frame_internal_mut(&mut self) -> &mut TransposerMetaData<T, S> {
        // SAFETY: this is good as long as the constructor's criteria are met.
        unsafe { self.frame_internal.as_mut().unwrap() }
    }
}

impl<'a, T: Transposer, S: StorageFamily, C: OutputCollector<T::Output>> InputStateContext<'a, T>
    for StepUpdateContext<T, S, C>
{
    fn get_input_state(&mut self) -> Pin<Box<dyn 'a + Future<Output = &'a T::InputState>>> {
        Box::pin(unsafe { self.input_state.as_ref().unwrap() })
    }
}

impl<T: Transposer, S: StorageFamily, C: OutputCollector<T::Output>> ScheduleEventContext<T>
    for StepUpdateContext<T, S, C>
{
    fn schedule_event(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<(), ScheduleEventError> {
        if time < self.time.raw_time() {
            return Err(ScheduleEventError::NewEventBeforeCurrent)
        }

        let time = self.time.spawn_scheduled(time, self.current_emission_index);

        self.get_frame_internal_mut().schedule_event(time, payload);
        self.current_emission_index += 1;

        Ok(())
    }

    fn schedule_event_expireable(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<ExpireHandle, ScheduleEventError> {
        if time < self.time.raw_time() {
            return Err(ScheduleEventError::NewEventBeforeCurrent)
        }

        let time = self.time.spawn_scheduled(time, self.current_emission_index);

        let handle = self
            .get_frame_internal_mut()
            .schedule_event_expireable(time, payload);
        self.current_emission_index += 1;

        Ok(handle)
    }
}

impl<T: Transposer, S: StorageFamily, C: OutputCollector<T::Output>> ExpireEventContext<T>
    for StepUpdateContext<T, S, C>
{
    fn expire_event(
        &mut self,
        handle: ExpireHandle,
    ) -> Result<(T::Time, T::Scheduled), ExpireEventError> {
        self.get_frame_internal_mut().expire_event(handle)
    }
}

impl<T: Transposer, S: StorageFamily, C: OutputCollector<T::Output>> EmitEventContext<T>
    for StepUpdateContext<T, S, C>
{
    fn emit_event(&mut self, payload: T::Output) {
        self.output_collector.push(payload);
    }
}

impl<T: Transposer, S: StorageFamily, C: OutputCollector<T::Output>> RngContext
    for StepUpdateContext<T, S, C>
{
    fn get_rng(&mut self) -> &mut dyn rand::RngCore {
        &mut self.get_frame_internal_mut().rng
    }
}
