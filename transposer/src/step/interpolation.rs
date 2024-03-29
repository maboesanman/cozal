use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use super::interpolate_context::StepInterpolateContext;
use super::wrapped_transposer::WrappedTransposer;
use super::InputState;
use crate::schedule_storage::{RefCounted, StorageFamily};
use crate::Transposer;

pub struct Interpolation<T: Transposer, Is: InputState<T>, S: StorageFamily> {
    future:      Pin<Box<dyn Future<Output = T::OutputState>>>,
    input_state: S::LazyState<Is>,
}

async fn create_fut<T: Transposer, S: StorageFamily, Is: InputState<T>>(
    interpolation_time: T::Time,
    wrapped_transposer: S::Transposer<WrappedTransposer<T, S>>,
    input_state: S::LazyState<Is>,
) -> T::OutputState {
    let borrowed = wrapped_transposer.borrow();
    let transposer = &borrowed.transposer;
    let metadata = &borrowed.metadata;

    let input_state_manager = input_state.get_provider();

    let mut context =
        StepInterpolateContext::new(interpolation_time, metadata, input_state_manager);

    transposer.interpolate(&mut context).await
}

impl<T: Transposer, Is: InputState<T>, S: StorageFamily> Interpolation<T, Is, S> {
    pub(crate) fn new(
        interpolation_time: T::Time,
        wrapped_transposer: S::Transposer<WrappedTransposer<T, S>>,
    ) -> Self {
        let input_state = S::LazyState::new(Box::new(Is::new()));

        let future =
            create_fut::<T, S, Is>(interpolation_time, wrapped_transposer, input_state.clone());

        let future: Pin<Box<dyn '_ + Future<Output = T::OutputState>>> = Box::pin(future);
        let future: Pin<Box<dyn 'static + Future<Output = T::OutputState>>> =
            unsafe { core::mem::transmute(future) };

        Self {
            future,
            input_state,
        }
    }

    pub fn get_input_state(&self) -> &Is {
        &self.input_state
    }
}

impl<T: Transposer, Is: InputState<T>, S: StorageFamily> Future for Interpolation<T, Is, S> {
    type Output = T::OutputState;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.future.as_mut().poll(cx)
    }
}
