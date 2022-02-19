use super::{SubStepTime, TransposerMetaData};
use crate::transposer::context::*;
use crate::transposer::schedule_storage::StorageFamily;
use crate::transposer::step::lazy_state::LazyState;
use crate::transposer::Transposer;

pub trait UpdateContext<T: Transposer, S: StorageFamily>:
    InitContext<'static, T> + HandleInputContext<'static, T> + HandleScheduleContext<'static, T>
{
    type Outputs;

    // SAFETY: ensure this UpdateContext is dropped before metadata.
    unsafe fn new(
        time: SubStepTime<T::Time>,
        metadata: *mut TransposerMetaData<T, S>,
        input_state: S::LazyState<LazyState<T::InputState>>,
    ) -> Self;

    fn recover_outputs(self) -> Self::Outputs;
}
