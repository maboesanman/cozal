use archery::SharedPointerKind;

// use super::lazy_state::LazyState;
use super::transposer_metadata::TransposerMetaData;
use crate::context::{InputStateContext, InterpolateContext};
use crate::Transposer;

pub struct StepInterpolateContext<'update, T: Transposer, P: SharedPointerKind> {
    // eventually may want this for accessing stuff like what the event schedule looks like
    _metadata:   &'update TransposerMetaData<T, P>,
    input_state: &'update T::InputStateManager,
}

impl<'update, T: Transposer, P: SharedPointerKind> StepInterpolateContext<'update, T, P> {
    pub fn new(
        metadata: &'update TransposerMetaData<T, P>,
        input_state: &'update T::InputStateManager,
    ) -> Self {
        Self {
            _metadata: metadata,
            input_state,
        }
    }
}

impl<'update, T: Transposer, P: SharedPointerKind> InterpolateContext<'update, T>
    for StepInterpolateContext<'update, T, P>
{
}

impl<'update, T: Transposer, P: SharedPointerKind> InputStateContext<'update, T>
    for StepInterpolateContext<'update, T, P>
{
    fn get_input_state_manager(&mut self) -> &'update T::InputStateManager {
        self.input_state
    }
}
