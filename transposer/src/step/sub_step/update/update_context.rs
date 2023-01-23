use super::{SubStepTime, TransposerMetaData};
use crate::context::*;
use crate::schedule_storage::StorageFamily;
use crate::Transposer;

pub trait UpdateContextFamily<T: Transposer, S: StorageFamily>
{
    type UpdateContext<'update>: UpdateContext<'update, T, S> + 'update
    where (T, S): 'update;
}

pub trait UpdateContext<'update, T: Transposer, S: StorageFamily>:
    InitContext<'update, T> + HandleInputContext<'update, T> + HandleScheduleContext<'update, T>
{
    // SAFETY: ensure this UpdateContext is dropped before metadata and input_state.
    fn new(
        time: SubStepTime<T::Time>,
        metadata: &'update mut TransposerMetaData<T, S>,
        input_state: &'update T::InputStateManager,
        output_sender: futures_channel::mpsc::Sender<(
            T::OutputEvent,
            futures_channel::oneshot::Sender<()>
        )>
    ) -> Self;
}