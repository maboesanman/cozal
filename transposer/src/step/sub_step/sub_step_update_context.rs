use core::future::Future;
use core::ops::Deref;
use core::pin::Pin;
use core::ptr::NonNull;

use super::time::SubStepTime;
use super::update::{TransposerMetaData, UpdateContext};
use crate::context::*;
use crate::schedule_storage::StorageFamily;
use crate::step::lazy_state::LazyState;
use crate::{ExpireHandle, Transposer};

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
pub struct SubStepUpdateContext<T: Transposer, S: StorageFamily, C: OutputCollector<T::Output>> {
    // these are pointers because this is stored next to the targets.
    metadata: NonNull<TransposerMetaData<T, S>>,

    time:                   SubStepTime<T::Time>,
    current_emission_index: usize,

    // values to output
    output_collector: C,

    input_state: S::LazyState<LazyState<T::InputState>>,
}

impl<'a, T: Transposer, S: StorageFamily, C: OutputCollector<T::Output>> InitContext<'a, T>
    for SubStepUpdateContext<T, S, C>
{
}
impl<'a, T: Transposer, S: StorageFamily, C: OutputCollector<T::Output>> HandleInputContext<'a, T>
    for SubStepUpdateContext<T, S, C>
{
}
impl<'a, T: Transposer, S: StorageFamily, C: OutputCollector<T::Output>>
    HandleScheduleContext<'a, T> for SubStepUpdateContext<T, S, C>
{
}
impl<T: Transposer, S: StorageFamily, C: OutputCollector<T::Output>> UpdateContext<T, S>
    for SubStepUpdateContext<T, S, C>
{
    type Outputs = C;

    // SAFETY: need to gurantee the metadata pointer outlives this object.
    unsafe fn new(
        time: SubStepTime<T::Time>,
        metadata: NonNull<TransposerMetaData<T, S>>,
        input_state: S::LazyState<LazyState<T::InputState>>,
    ) -> Self {
        Self {
            metadata,
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

impl<T: Transposer, S: StorageFamily, C: OutputCollector<T::Output>> SubStepUpdateContext<T, S, C> {
    fn get_metadata_mut(&mut self) -> &mut TransposerMetaData<T, S> {
        // SAFETY: this is good as long as the constructor's criteria are met.
        unsafe { self.metadata.as_mut() }
    }
}

impl<'a, T: Transposer, S: StorageFamily, C: OutputCollector<T::Output>> InputStateContext<'a, T>
    for SubStepUpdateContext<T, S, C>
{
    fn get_input_state(&mut self) -> Pin<Box<dyn 'a + Future<Output = &'a T::InputState>>> {
        let ptr: NonNull<_> = self.input_state.deref().into();

        // SAFETY: 'a is scoped to the transposer's handler future, which must outlive this scope
        // because that's where this function gets called from.
        Box::pin(unsafe { ptr.as_ref() })
    }
}

impl<T: Transposer, S: StorageFamily, C: OutputCollector<T::Output>> ScheduleEventContext<T>
    for SubStepUpdateContext<T, S, C>
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

        self.get_metadata_mut().schedule_event(time, payload);
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
            .get_metadata_mut()
            .schedule_event_expireable(time, payload);
        self.current_emission_index += 1;

        Ok(handle)
    }
}

impl<T: Transposer, S: StorageFamily, C: OutputCollector<T::Output>> ExpireEventContext<T>
    for SubStepUpdateContext<T, S, C>
{
    fn expire_event(
        &mut self,
        handle: ExpireHandle,
    ) -> Result<(T::Time, T::Scheduled), ExpireEventError> {
        self.get_metadata_mut().expire_event(handle)
    }
}

impl<T: Transposer, S: StorageFamily, C: OutputCollector<T::Output>> EmitEventContext<T>
    for SubStepUpdateContext<T, S, C>
{
    fn emit_event(&mut self, payload: T::Output) {
        self.output_collector.push(payload);
    }
}

impl<T: Transposer, S: StorageFamily, C: OutputCollector<T::Output>> RngContext
    for SubStepUpdateContext<T, S, C>
{
    fn get_rng(&mut self) -> &mut dyn rand::RngCore {
        &mut self.get_metadata_mut().rng
    }
}
