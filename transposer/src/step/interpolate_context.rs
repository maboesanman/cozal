use super::time::SubStepTime;
// use super::lazy_state::LazyState;
use super::transposer_metadata::TransposerMetaData;
use crate::context::{
    CurrentTimeContext,
    InputStateContext,
    InterpolateContext,
    LastUpdatedTimeContext,
};
use crate::schedule_storage::StorageFamily;
use crate::Transposer;

pub struct StepInterpolateContext<'update, T: Transposer, S: StorageFamily> {
    interpolation_time: T::Time,
    metadata:           &'update TransposerMetaData<T, S>,
    input_state:        &'update T::InputStateManager,
}

impl<'update, T: Transposer, S: StorageFamily> StepInterpolateContext<'update, T, S> {
    pub fn new(
        interpolation_time: T::Time,
        metadata: &'update TransposerMetaData<T, S>,
        input_state: &'update T::InputStateManager,
    ) -> Self {
        Self {
            interpolation_time,
            metadata,
            input_state,
        }
    }
}

impl<'update, T: Transposer, S: StorageFamily> InterpolateContext<'update, T>
    for StepInterpolateContext<'update, T, S>
{
}

impl<'update, T: Transposer, S: StorageFamily> InputStateContext<'update, T>
    for StepInterpolateContext<'update, T, S>
{
    fn get_input_state_manager(&mut self) -> &'update T::InputStateManager {
        self.input_state
    }
}

impl<'update, T: Transposer, S: StorageFamily> CurrentTimeContext<T>
    for StepInterpolateContext<'update, T, S>
{
    fn current_time(&self) -> <T as Transposer>::Time {
        self.interpolation_time
    }
}

impl<'update, T: Transposer, S: StorageFamily> LastUpdatedTimeContext<T>
    for StepInterpolateContext<'update, T, S>
{
    fn last_updated_time(&self) -> <T as Transposer>::Time {
        self.metadata.last_updated.time
    }
}
