use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use futures_core::Future;

use super::pointer_interpolation::PointerInterpolation;
use super::step::WrappedTransposer;
use crate::transposer::schedule_storage::StorageFamily;
use crate::transposer::Transposer;

pub struct Interpolation<'t, T: Transposer, S: StorageFamily> {
    inner: PointerInterpolation<T>,

    // this forces the lifetime to be valid
    phantom: PhantomData<&'t WrappedTransposer<T, S>>,
}

impl<'t, T: Transposer, S: StorageFamily> Interpolation<'t, T, S> {
    pub fn time(&self) -> T::Time {
        self.inner.time()
    }

    // SAFETY: wrapped transposer must outlive this object.
    pub(crate) unsafe fn new(
        time: T::Time,
        wrapped_transposer: &'t WrappedTransposer<T, S>,
    ) -> Self {
        Self {
            inner:   unsafe { PointerInterpolation::new(time, wrapped_transposer) },
            phantom: PhantomData,
        }
    }

    pub fn needs_state(&self) -> bool {
        self.inner.context.state.requested()
    }

    pub fn set_state(
        &self,
        state: T::InputState,
        ignore_waker: &Waker,
    ) -> Result<(), Box<T::InputState>> {
        self.inner.context.state.set(state, ignore_waker)
    }
}

impl<'t, T: Transposer, S: StorageFamily> Future for Interpolation<'t, T, S> {
    type Output = T::OutputState;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.inner.future.as_mut().poll(cx)
    }
}
