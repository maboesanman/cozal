use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use futures_core::Future;

use super::interpolate_context::StepGroupInterpolateContext;
use super::step::WrappedTransposer;
use crate::transposer::schedule_storage::StorageFamily;
use crate::transposer::Transposer;
use crate::util::dummy_waker::DummyWaker;

pub struct PointerInterpolation<T: Transposer> {
    pub(crate) future:  Pin<Box<dyn Future<Output = T::OutputState>>>,
    pub(crate) context: Box<StepGroupInterpolateContext<T>>,
    time:               T::Time,
    waker:              Waker,
}

impl<T: Transposer> PointerInterpolation<T> {
    pub fn wake(&self) {
        self.waker.wake_by_ref()
    }

    pub fn time(&self) -> T::Time {
        self.time
    }

    // SAFETY: wrapped transposer must outlive this object.
    pub unsafe fn new<S: StorageFamily>(
        time: T::Time,
        wrapped_transposer: &WrappedTransposer<T, S>,
    ) -> Self {
        let mut context = Box::new(StepGroupInterpolateContext::new());
        let context_ptr: *mut _ = context.as_mut();
        let context_ref = unsafe { context_ptr.as_mut().unwrap() };

        let base_time = wrapped_transposer.metadata.last_updated;
        let transposer = &wrapped_transposer.transposer;
        let future = T::interpolate(transposer, base_time, time, context_ref);
        let future: Pin<Box<dyn Future<Output = T::OutputState>>> =
            unsafe { core::mem::transmute(future) };

        Self {
            time,
            context,
            future,
            waker: DummyWaker::dummy(),
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
