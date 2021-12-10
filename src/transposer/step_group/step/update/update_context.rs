use super::{StepTime, TransposerMetaData};
use crate::transposer::context::*;
use crate::transposer::step_group::lazy_state::LazyState;
use crate::transposer::Transposer;

pub trait UpdateContext<T: Transposer>:
    InitContext<'static, T> + HandleInputContext<'static, T> + HandleScheduleContext<'static, T>
where
    T::Scheduled: Clone,
{
    type Outputs;

    // SAFETY: ensure this UpdateContext is dropped before frame_internal and input_state.
    unsafe fn new(
        time: StepTime<T::Time>,
        metadata: *mut TransposerMetaData<T>,
        input_state: *const LazyState<T::InputState>,
    ) -> Self;

    fn recover_outputs(self) -> Self::Outputs;
}
