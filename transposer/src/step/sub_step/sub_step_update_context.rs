use core::future::Future;
use core::ops::Deref;
use core::pin::Pin;
use core::ptr::NonNull;

use super::time::SubStepTime;
use super::update::{TransposerMetaData, UpdateContext};
use crate::context::*;
use crate::schedule_storage::StorageFamily;
use crate::{ExpireHandle, Transposer};

pub trait OutputCollector<O> {
    fn new() -> Self;
    async fn set(&mut self, output: O);
    fn take(&mut self) -> Option<O>;
}

pub enum AsyncCollector<O> {
    Some {
        output: O,
        notify: futures_channel::oneshot::Sender<()>,
    },
    None,
}

impl<O> OutputCollector<O> for AsyncCollector<O> {
    fn new() -> Self {
        Self::None
    }
    async fn set(&mut self, output: O) {
        let (notify, recv) = futures_channel::oneshot::channel();
        debug_assert!(matches!(self, AsyncCollector::None));

        *self = AsyncCollector::Some {
            output,
            notify,
        };

        recv.await.unwrap()
    }
    fn take(&mut self) -> Option<O> {
        match core::mem::replace(self, AsyncCollector::None) {
            AsyncCollector::None => None,
            AsyncCollector::Some {
                output,
                notify,
            } => {
                let _ = notify.send(());
                Some(output)
            },
        }
    }
}

pub struct DiscardCollector;

impl<O> OutputCollector<O> for DiscardCollector {
    fn new() -> Self {
        DiscardCollector
    }
    async fn set(&mut self, output: O) {
        drop(output)
    }
    fn take(&mut self) -> Option<O> {
        None
    }
}

/// This is the interface through which you can do a variety of functions in your transposer.
///
/// the primary features are scheduling and expiring events,
/// though there are more methods to interact with the engine.
pub struct SubStepUpdateContext<T: Transposer, S: StorageFamily, C: OutputCollector<T::OutputEvent>> {
    // these are pointers because this is stored next to the targets.
    metadata: NonNull<TransposerMetaData<T, S>>,

    time:                   SubStepTime<T::Time>,
    current_emission_index: usize,

    // values to output
    output_collector: C,

    input_state: NonNull<T::InputStateProvider>,
}

impl<'a, T: Transposer, S: StorageFamily, C: OutputCollector<T::OutputEvent>> InitContext<'a, T>
    for SubStepUpdateContext<T, S, C>
{
}
impl<'a, T: Transposer, S: StorageFamily, C: OutputCollector<T::OutputEvent>> HandleInputContext<'a, T>
    for SubStepUpdateContext<T, S, C>
{
}
impl<'a, T: Transposer, S: StorageFamily, C: OutputCollector<T::OutputEvent>>
    HandleScheduleContext<'a, T> for SubStepUpdateContext<T, S, C>
{
}
impl<T: Transposer, S: StorageFamily, C: OutputCollector<T::OutputEvent>> UpdateContext<T, S>
    for SubStepUpdateContext<T, S, C>
{
    type Output = C;

    // SAFETY: need to gurantee the metadata pointer outlives this object.
    unsafe fn new(
        time: SubStepTime<T::Time>,
        metadata: NonNull<TransposerMetaData<T, S>>,
        input_state: NonNull<T::InputStateProvider>,
    ) -> Self {
        Self {
            metadata,
            input_state,
            time,
            current_emission_index: 0,
            output_collector: C::new(),
        }
    }

    fn recover_output(&mut self) -> Option<T::OutputEvent> {
        OutputCollector::take(&mut self.output_collector)
    }
}

impl<T: Transposer, S: StorageFamily, C: OutputCollector<T::OutputEvent>> SubStepUpdateContext<T, S, C> {
    fn get_metadata_mut(&mut self) -> &mut TransposerMetaData<T, S> {
        // SAFETY: this is good as long as the constructor's criteria are met.
        unsafe { self.metadata.as_mut() }
    }
}

impl<'a, T: Transposer, S: StorageFamily, C: OutputCollector<T::OutputEvent>> InputStateContext<'a, T>
    for SubStepUpdateContext<T, S, C>
{
    fn get_input_state_requester(&mut self) -> &'a mut T::InputStateProvider {
        unsafe { self.input_state.as_mut() }
    }
}

impl<T: Transposer, S: StorageFamily, C: OutputCollector<T::OutputEvent>> ScheduleEventContext<T>
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

impl<T: Transposer, S: StorageFamily, C: OutputCollector<T::OutputEvent>> ExpireEventContext<T>
    for SubStepUpdateContext<T, S, C>
{
    fn expire_event(
        &mut self,
        handle: ExpireHandle,
    ) -> Result<(T::Time, T::Scheduled), ExpireEventError> {
        self.get_metadata_mut().expire_event(handle)
    }
}

impl<T: Transposer, S: StorageFamily, C: OutputCollector<T::OutputEvent>> EmitEventContext<T>
    for SubStepUpdateContext<T, S, C>
{
    fn emit_event(&mut self, payload: <T as Transposer>::OutputEvent) -> Pin<Box<dyn '_ + Future<Output = ()>>> {
        Box::pin(self.output_collector.set(payload))
    }
}

impl<T: Transposer, S: StorageFamily, C: OutputCollector<T::OutputEvent>> RngContext
    for SubStepUpdateContext<T, S, C>
{
    fn get_rng(&mut self) -> &mut dyn rand::RngCore {
        &mut self.get_metadata_mut().rng
    }
}
