use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use futures_core::Future;

use super::interpolate_context::StepInterpolateContext;
use super::sub_step::WrappedTransposer;
use crate::transposer::schedule_storage::StorageFamily;
use crate::transposer::Transposer;

pub struct PointerInterpolation<T: Transposer> {
    pub(crate) future:  Pin<Box<dyn Future<Output = T::OutputState>>>,
    pub(crate) context: Box<StepInterpolateContext<T>>,
}

impl<T: Transposer> PointerInterpolation<T> {
    // SAFETY: wrapped transposer must outlive this object.
    pub unsafe fn new<S: StorageFamily>(
        time: T::Time,
        wrapped_transposer: &WrappedTransposer<T, S>,
    ) -> Self {
        let mut context = Box::new(StepInterpolateContext::new());
        let context_ptr: *mut _ = context.as_mut();
        let context_ref = unsafe { context_ptr.as_mut().unwrap() };

        let base_time = wrapped_transposer.metadata.last_updated;
        let transposer = &wrapped_transposer.transposer;
        let future = T::interpolate(transposer, base_time, time, context_ref);
        let future: Pin<Box<dyn Future<Output = T::OutputState>>> =
            unsafe { core::mem::transmute(future) };

        Self {
            context,
            future,
        }
    }

    pub fn needs_state(&self) -> bool {
        self.context.state.requested()
    }

    pub fn set_state(
        &self,
        state: T::InputState,
        ignore_waker: &Waker,
    ) -> Result<(), Box<T::InputState>> {
        self.context.state.set(state, ignore_waker)
    }
}

impl<T: Transposer> Future for PointerInterpolation<T> {
    type Output = T::OutputState;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.future.as_mut().poll(cx)
    }
}
