use core::future::Future;
use core::pin::Pin;
use core::ptr::NonNull;
use core::task::{Context, Poll};

use super::interpolate_context::StepInterpolateContext;
use super::sub_step::WrappedTransposer;
use crate::schedule_storage::{StorageFamily, RefCounted};
use crate::Transposer;

pub struct Interpolation<T: Transposer, S: StorageFamily> {
    future:  Pin<Box<dyn Future<Output = T::OutputState>>>,
    context: Box<StepInterpolateContext<T>>,

    // this is referenced by future and context. it's not technically used,
    // except to keep alive the references until dropped.
    #[allow(unused)]
    wrapped_transposer: <S::Transposer<WrappedTransposer<T, S>> as RefCounted<
        WrappedTransposer<T, S>,
    >>::Borrowed,
}

impl<T: Transposer, S: StorageFamily> Interpolation<T, S> {
    pub fn new(time: T::Time, wrapped_transposer: &S::Transposer<WrappedTransposer<T, S>>) -> Self {
        let borrowed = wrapped_transposer.borrow();
        let mut context = Box::new(StepInterpolateContext::new());
        let mut context_ptr: NonNull<_> = context.as_mut().into();

        // SAFETY: this is owned by future, so it will not dangle as future is dropped before context.
        let context_ref = unsafe { context_ptr.as_mut() };

        let base_time = borrowed.metadata.last_updated;
        let transposer = &borrowed.transposer;
        let future = T::interpolate(transposer, base_time, time, context_ref);

        // SAFETY: forcing the lifetime. This is dropped before the borrowed content (context) so its fine.
        let future: Pin<Box<dyn Future<Output = T::OutputState>>> = future;
        let future: Pin<Box<dyn Future<Output = T::OutputState>>> =
            unsafe { core::mem::transmute(future) };

        Self {
            future,
            context,
            wrapped_transposer: borrowed,
        }
    }

    pub fn needs_state(&self) -> bool {
        self.context.state.requested()
    }

    // pub fn set_state(&mut self, state: T::InputState) -> Result<(), Arc<T::InputState>> {
    //     self.context.state.set(state)
    // }
}

impl<T: Transposer, S: StorageFamily> Future for Interpolation<T, S> {
    type Output = T::OutputState;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.future.as_mut().poll(cx)
    }
}
