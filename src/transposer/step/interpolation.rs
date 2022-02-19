use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use futures_core::Future;

use super::interpolate_context::StepInterpolateContext;
use super::sub_step::WrappedTransposer;
use crate::transposer::schedule_storage::{StorageFamily, TransposerPointer};
use crate::transposer::Transposer;

pub struct Interpolation<T: Transposer, S: StorageFamily> {
    future:  Pin<Box<dyn Future<Output = T::OutputState>>>,
    context: Box<StepInterpolateContext<T>>,

    wrapped_transposer: <S::Transposer<WrappedTransposer<T, S>> as TransposerPointer<
        WrappedTransposer<T, S>,
    >>::Borrowed,
}

impl<T: Transposer, S: StorageFamily> Interpolation<T, S> {
    pub fn new(time: T::Time, wrapped_transposer: &S::Transposer<WrappedTransposer<T, S>>) -> Self {
        let borrowed = wrapped_transposer.borrow();
        let mut context = Box::new(StepInterpolateContext::new());
        let context_ptr: *mut _ = context.as_mut();
        let context_ref = unsafe { context_ptr.as_mut().unwrap() };

        let base_time = borrowed.metadata.last_updated;
        let transposer = &borrowed.transposer;
        let future = T::interpolate(transposer, base_time, time, context_ref);
        let future: Pin<Box<dyn Future<Output = T::OutputState>>> =
            unsafe { core::mem::transmute(future) };

        Self {
            context,
            future,
            wrapped_transposer: borrowed,
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

impl<T: Transposer, S: StorageFamily> Future for Interpolation<T, S> {
    type Output = T::OutputState;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.future.as_mut().poll(cx)
    }
}
