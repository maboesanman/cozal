use std::hint::unreachable_unchecked;
use std::marker::PhantomPinned;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use futures_core::Future;

use super::interpolate_context::StepGroupInterpolateContext;
use super::lazy_state::LazyState;
use super::step::WrappedTransposer;
use crate::transposer::Transposer;

pub struct PointerInterpolation<T: Transposer> {
    inner:              InterpolationInner<T>,
    state:              LazyState<T::InputState>,
    wrapped_transposer: *const WrappedTransposer<T>,
}

enum InterpolationInner<T: Transposer> {
    Pending {
        time: T::Time,
    },
    Active {
        future:  MaybeUninit<Pin<Box<dyn Future<Output = T::OutputState>>>>,
        context: StepGroupInterpolateContext<T>,
        time:    T::Time,
        waker:   Waker,

        pinned: PhantomPinned,
    },
}

impl<T: Transposer> PointerInterpolation<T> {
    pub fn wake(&self) {
        if let InterpolationInner::Active {
            waker, ..
        } = &self.inner
        {
            waker.wake_by_ref()
        }
    }

    pub fn time(&self) -> T::Time {
        match self.inner {
            InterpolationInner::Pending {
                time, ..
            } => time,
            InterpolationInner::Active {
                time, ..
            } => time,
        }
    }

    pub fn new(time: T::Time, wrapped_transposer: *const WrappedTransposer<T>) -> Self {
        Self {
            inner: InterpolationInner::Pending {
                time,
            },
            state: LazyState::new(),
            wrapped_transposer,
        }
    }

    pub fn needs_state(&self) -> bool {
        self.state.requested()
    }

    pub fn set_state(
        &self,
        state: T::InputState,
        ignore_waker: &Waker,
    ) -> Result<(), Box<T::InputState>> {
        self.state.set(state, ignore_waker)
    }
}

impl<T: Transposer> Drop for InterpolationInner<T> {
    fn drop(&mut self) {
        if let InterpolationInner::Active {
            future, ..
        } = self
        {
            unsafe { future.assume_init_drop() };
        }
    }
}

impl<T: Transposer> Future for PointerInterpolation<T> {
    type Output = T::OutputState;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        let wrapped_transposer = this.wrapped_transposer;
        let state = &this.state;
        if let InterpolationInner::Pending {
            time,
        } = this.inner
        {
            this.inner = InterpolationInner::Active {
                future: MaybeUninit::uninit(),
                context: StepGroupInterpolateContext::new(state),
                time,
                waker: cx.waker().clone(),
                pinned: PhantomPinned,
            };

            match &mut this.inner {
                InterpolationInner::Pending {
                    ..
                } => unsafe { unreachable_unchecked() },
                InterpolationInner::Active {
                    future,
                    context,
                    time,
                    ..
                } => {
                    let context: *mut _ = context;
                    let context = unsafe { context.as_mut().unwrap() };
                    let wrapped_transposer = unsafe { wrapped_transposer.as_ref().unwrap() };
                    let base_time = wrapped_transposer.metadata.last_updated;
                    let transposer = &wrapped_transposer.transposer;
                    let fut = T::interpolate(transposer, base_time, *time, context);
                    let fut = unsafe { core::mem::transmute(fut) };
                    *future = MaybeUninit::new(fut);
                },
            };
        }

        match &mut this.inner {
            InterpolationInner::Pending {
                ..
            } => unsafe { unreachable_unchecked() },
            InterpolationInner::Active {
                future, ..
            } => {
                let f = unsafe { future.assume_init_mut() };
                f.as_mut().poll(cx)
            },
        }
    }
}
