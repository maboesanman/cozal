use core::future::Future;
use core::pin::Pin;
use core::ptr::NonNull;

use super::lazy_state::LazyState;
use crate::context::{InputStateContext, InterpolateContext};
use crate::Transposer;

pub struct StepInterpolateContext<T: Transposer> {
    pub state: LazyState<T::InputState>,
}

impl<T: Transposer> StepInterpolateContext<T> {
    pub fn new() -> Self {
        Self {
            state: LazyState::new(),
        }
    }
}

impl<'a, T: Transposer> InterpolateContext<'a, T> for StepInterpolateContext<T> {}

impl<'a, T: Transposer> InputStateContext<'a, T> for StepInterpolateContext<T> {
    fn get_input_state(&mut self) -> Pin<Box<dyn 'a + Future<Output = &'a T::InputState>>> {
        let state_ptr = NonNull::from(&self.state);

        // SAFETY: 'a is scoped to the transposer's handler future, which must outlive this scope
        // because that's where this function gets called from.
        Box::pin(unsafe { state_ptr.as_ref() })
    }
}
