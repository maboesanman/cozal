use core::future::Future;
use core::pin::Pin;
use core::ptr::NonNull;
use core::task::{Context, Poll};
use std::marker::PhantomData;

use super::interpolate_context::StepInterpolateContext;
use super::sub_step::WrappedTransposer;
use super::InputState;
use crate::context::InterpolateContext;
use crate::schedule_storage::{RefCounted, StorageFamily};
use crate::Transposer;

pub struct Interpolation<'almost_static, T: Transposer, Is: InputState<T>, S: StorageFamily>
where
    (T, Is): 'almost_static,
{
    future:      Pin<Box<dyn 'almost_static + Future<Output = T::OutputState>>>,
    input_state: S::LazyState<Is>,
}

async fn create_fut<T: Transposer, S: StorageFamily, Is: InputState<T>>(
    wrapped_transposer: S::Transposer<WrappedTransposer<T, S>>,
    base_time: T::Time,
    interpolated_time: T::Time,
    input_state: S::LazyState<Is>,
) -> T::OutputState {
    let borrowed = wrapped_transposer.borrow();
    let transposer = &borrowed.transposer;
    let metadata = &borrowed.metadata;

    let input_state_manager = input_state.get_provider();

    let mut context = StepInterpolateContext::new(metadata, input_state_manager);

    transposer
        .interpolate(base_time, interpolated_time, &mut context)
        .await
}

impl<'almost_static, T: Transposer, Is: InputState<T>, S: StorageFamily>
    Interpolation<'almost_static, T, Is, S>
{
    pub(crate) fn new(
        base_time: T::Time,
        interpolated_time: T::Time,
        wrapped_transposer: S::Transposer<WrappedTransposer<T, S>>,
    ) -> Self {
        let input_state = S::LazyState::new(Box::new(Is::new()));

        let future = Box::pin(create_fut::<T, S, Is>(
            wrapped_transposer,
            base_time,
            interpolated_time,
            input_state.clone(),
        ));

        Self {
            future,
            input_state,
        }
    }

    pub fn get_input_state(&self) -> &Is {
        &self.input_state
    }
}

impl<'almost_static, T: Transposer, Is: InputState<T>, S: StorageFamily> Future
    for Interpolation<'almost_static, T, Is, S>
{
    type Output = T::OutputState;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.future.as_mut().poll(cx)
    }
}
