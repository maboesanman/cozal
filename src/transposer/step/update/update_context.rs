use super::{LazyState, StepTime, TransposerMetaData};
use crate::transposer::context::*;
use crate::transposer::Transposer;

pub trait UpdateContext<T: Transposer>:
    InitContext<T> + HandleInputContext<T> + HandleScheduleContext<T>
where
    T::Scheduled: Clone,
{
    type Outputs;

    // SAFETY: ensure this UpdateContext is dropped before frame_internal and input_state.
    unsafe fn new(
        time: StepTime<T::Time>,
        metadata: *mut TransposerMetaData<T>,
        input_state: *mut LazyState<T::InputState>,
    ) -> Self;

    fn recover_outputs(self) -> Self::Outputs;
}
