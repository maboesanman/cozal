use core::future::Future;
use core::ops::Deref;
use core::pin::Pin;
use core::ptr::NonNull;
use std::marker::PhantomData;

use super::time::SubStepTime;
use super::update::{TransposerMetaData, UpdateContext, UpdateContextFamily};
use crate::context::*;
use crate::schedule_storage::StorageFamily;
use crate::{ExpireHandle, Transposer};

// pub trait OutputCollector<O> {
//     fn new() -> Self;
//     async fn set(&mut self, output: O);
//     fn take(&mut self) -> Option<O>;
// }

// pub enum AsyncCollector<O> {
//     Some {
//         output: O,
//         notify: futures_channel::oneshot::Sender<()>,
//     },
//     None,
// }

// impl<O> OutputCollector<O> for AsyncCollector<O> {
//     fn new() -> Self {
//         Self::None
//     }
//     async fn set(&mut self, output: O) {
//         let (notify, recv) = futures_channel::oneshot::channel();
//         debug_assert!(matches!(self, AsyncCollector::None));

//         *self = AsyncCollector::Some {
//             output,
//             notify,
//         };

//         recv.await.unwrap()
//     }
//     fn take(&mut self) -> Option<O> {
//         match core::mem::replace(self, AsyncCollector::None) {
//             AsyncCollector::None => None,
//             AsyncCollector::Some {
//                 output,
//                 notify,
//             } => {
//                 let _ = notify.send(());
//                 Some(output)
//             },
//         }
//     }
// }

// pub struct DiscardCollector;

// impl<O> OutputCollector<O> for DiscardCollector {
//     fn new() -> Self {
//         DiscardCollector
//     }
//     async fn set(&mut self, output: O) {
//         drop(output)
//     }
//     fn take(&mut self) -> Option<O> {
//         None
//     }
// }

/// This is the interface through which you can do a variety of functions in your transposer.
///
/// the primary features are scheduling and expiring events,
/// though there are more methods to interact with the engine.
pub struct SubStepUpdateContext<'update, T: Transposer, S: StorageFamily> {
    // these are pointers because this is stored next to the targets.
    metadata: &'update mut TransposerMetaData<T, S>,

    time:                   SubStepTime<T::Time>,
    current_emission_index: usize,

    // values to output
    output_sender:
        futures_channel::mpsc::Sender<(T::OutputEvent, futures_channel::oneshot::Sender<()>)>,

    input_state: &'update T::InputStateManager,
}

pub struct SubStepUpdateContextFamily<T: Transposer, S: StorageFamily>(PhantomData<(T, S)>);

impl<T: Transposer, S: StorageFamily> UpdateContextFamily<T, S>
    for SubStepUpdateContextFamily<T, S>
{
    type UpdateContext<'update> = SubStepUpdateContext<'update, T, S>
    where (T, S): 'update;
}

impl<'update, T: Transposer, S: StorageFamily> InitContext<'update, T>
    for SubStepUpdateContext<'update, T, S>
{
}
impl<'update, T: Transposer, S: StorageFamily> HandleInputContext<'update, T>
    for SubStepUpdateContext<'update, T, S>
{
}
impl<'update, T: Transposer, S: StorageFamily> HandleScheduleContext<'update, T>
    for SubStepUpdateContext<'update, T, S>
{
}
impl<'update, T: Transposer, S: StorageFamily> UpdateContext<'update, T, S>
    for SubStepUpdateContext<'update, T, S>
{
    // SAFETY: need to gurantee the metadata pointer outlives this object.
    fn new(
        time: SubStepTime<T::Time>,
        metadata: &'update mut TransposerMetaData<T, S>,
        input_state: &'update T::InputStateManager,
        output_sender: futures_channel::mpsc::Sender<(
            T::OutputEvent,
            futures_channel::oneshot::Sender<()>,
        )>,
    ) -> Self {
        Self {
            metadata,
            input_state,
            time,
            current_emission_index: 0,
            output_sender,
        }
    }
}

impl<'update, T: Transposer, S: StorageFamily> InputStateContext<'update, T>
    for SubStepUpdateContext<'update, T, S>
{
    fn get_input_state_manager(&mut self) -> &'update T::InputStateManager {
        self.input_state
    }
}

impl<'update, T: Transposer, S: StorageFamily> ScheduleEventContext<T>
    for SubStepUpdateContext<'update, T, S>
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

        self.metadata.schedule_event(time, payload);
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

        let handle = self.metadata.schedule_event_expireable(time, payload);
        self.current_emission_index += 1;

        Ok(handle)
    }
}

impl<'update, T: Transposer, S: StorageFamily> ExpireEventContext<T>
    for SubStepUpdateContext<'update, T, S>
{
    fn expire_event(
        &mut self,
        handle: ExpireHandle,
    ) -> Result<(T::Time, T::Scheduled), ExpireEventError> {
        self.metadata.expire_event(handle)
    }
}

impl<'update, T: Transposer, S: StorageFamily> EmitEventContext<T>
    for SubStepUpdateContext<'update, T, S>
{
    fn emit_event(
        &mut self,
        payload: <T as Transposer>::OutputEvent,
    ) -> Pin<Box<dyn '_ + Future<Output = ()>>> {
        let (send, recv) = futures_channel::oneshot::channel();
        self.output_sender.try_send((payload, send)).unwrap();

        Box::pin(async move {
            recv.await.unwrap();
        })
    }
}

impl<'update, T: Transposer, S: StorageFamily> RngContext for SubStepUpdateContext<'update, T, S> {
    fn get_rng(&mut self) -> &mut dyn rand::RngCore {
        &mut self.metadata.rng
    }
}
