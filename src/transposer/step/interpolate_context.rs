use std::pin::Pin;

use futures_core::Future;

use super::lazy_state::LazyState;
use crate::transposer::context::{InputStateContext, InterpolateContext};
use crate::transposer::Transposer;

pub struct StepGroupInterpolateContext<T: Transposer> {
    pub state: LazyState<T::InputState>,
}

impl<T: Transposer> StepGroupInterpolateContext<T> {
    pub fn new() -> Self {
        Self {
            state: LazyState::new(),
        }
    }
}

impl<'a, T: Transposer> InterpolateContext<'a, T> for StepGroupInterpolateContext<T> {}

impl<'a, T: Transposer> InputStateContext<'a, T> for StepGroupInterpolateContext<T> {
    fn get_input_state(&mut self) -> Pin<Box<dyn 'a + Future<Output = &'a T::InputState>>> {
        let state_ptr: *const _ = &self.state;
        Box::pin(unsafe { state_ptr.as_ref().unwrap() })
    }
}
