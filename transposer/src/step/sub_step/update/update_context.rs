use core::ptr::NonNull;

use super::{SubStepTime, TransposerMetaData};
use crate::context::*;
use crate::schedule_storage::StorageFamily;
use crate::Transposer;

pub trait UpdateContext<T: Transposer, S: StorageFamily>:
    InitContext<'static, T> + HandleInputContext<'static, T> + HandleScheduleContext<'static, T>
{
    type Output;

    // SAFETY: ensure this UpdateContext is dropped before metadata and input_state.
    unsafe fn new(
        time: SubStepTime<T::Time>,
        metadata: NonNull<TransposerMetaData<T, S>>,
        input_state: NonNull<T::InputStateProvider>,
    ) -> Self;

    fn recover_output(&mut self) -> Option<T::OutputEvent>;
}
