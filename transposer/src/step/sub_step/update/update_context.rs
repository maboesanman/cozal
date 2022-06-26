use core::ptr::NonNull;

use super::{SubStepTime, TransposerMetaData};
use crate::context::*;
use crate::schedule_storage::StorageFamily;
use crate::step::lazy_state::LazyState;
use crate::Transposer;

pub trait UpdateContext<T: Transposer, S: StorageFamily>:
    InitContext<'static, T> + HandleInputContext<'static, T> + HandleScheduleContext<'static, T>
{
    type Outputs;

    // SAFETY: ensure this UpdateContext is dropped before metadata.
    unsafe fn new(
        time: SubStepTime<T::Time>,
        metadata: NonNull<TransposerMetaData<T, S>>,
        input_state: S::LazyState<LazyState<T::InputState>>,
    ) -> Self;

    fn recover_outputs(self) -> Self::Outputs;
}
