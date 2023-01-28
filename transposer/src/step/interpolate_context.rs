// use super::lazy_state::LazyState;
use super::transposer_metadata::TransposerMetaData;
use crate::context::{InputStateContext, InterpolateContext};
use crate::schedule_storage::StorageFamily;
use crate::Transposer;

pub struct StepInterpolateContext<'update, T: Transposer, S: StorageFamily> {
    // eventually may want this for accessing stuff like what the event schedule looks like
    _metadata:   &'update TransposerMetaData<T, S>,
    input_state: &'update T::InputStateManager,
}

impl<'update, T: Transposer, S: StorageFamily> StepInterpolateContext<'update, T, S> {
    pub fn new(
        metadata: &'update TransposerMetaData<T, S>,
        input_state: &'update T::InputStateManager,
    ) -> Self {
        Self {
            _metadata: metadata,
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
