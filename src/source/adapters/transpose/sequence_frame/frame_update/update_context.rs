use super::{EngineTime, FrameMetaData, LazyState};
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
        time: EngineTime<T::Time>,
        metadata: *mut FrameMetaData<T>,
        input_state: *mut LazyState<T::InputState>,
    ) -> Self;

    fn recover_outputs(self) -> Self::Outputs;
}
