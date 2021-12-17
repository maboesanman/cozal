use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use super::{Arg, StepTime, UpdateContext, WrappedTransposer};
use crate::transposer::schedule_storage::StorageFamily;
use crate::transposer::step_group::lazy_state::LazyState;
use crate::transposer::Transposer;

pub struct Update<T: Transposer, S: StorageFamily, C: UpdateContext<T, S>, A: Arg<T, S>> {
    // references context, wrapped_transposer.transposer, and args
    future: Pin<Box<dyn Future<Output = ()>>>,

    arg: A::Stored,

    // references state and wrapped_transposer.internal
    context: Box<C>,

    // may be partially modified.
    // treated as borrowed, split between future and context.
    wrapped_transposer: Box<WrappedTransposer<T, S>>,
}

impl<T: Transposer, S: StorageFamily, C: UpdateContext<T, S>, A: Arg<T, S>> Update<T, S, C, A> {
    // SAFETY: input_state must outlive returned value
    pub unsafe fn new(
        mut wrapped_transposer: Box<WrappedTransposer<T, S>>,
        mut arg: A::Stored,
        time: StepTime<T::Time>,
        input_state: *const LazyState<T::InputState>,
    ) -> Self {
        // update 'current time'
        let raw_time = time.raw_time();
        wrapped_transposer.metadata.last_updated = raw_time;

        // // upgrade arg from borrowed to passed (pulling the event from the schedule if scheduled).
        let arg_passed = A::get_passed(&mut wrapped_transposer, &mut arg);

        // split borrow. one goes to fut, one to context.
        let transposer = &mut wrapped_transposer.transposer;
        let metadata = &mut wrapped_transposer.metadata;

        // SAFETY: metadata outlives context by drop order, input_state outlives because new is unsafe and the caller must uphold.
        let context = unsafe { C::new(time.clone(), metadata, input_state) };
        let mut context = Box::new(context);
        let context_mut: *mut _ = context.as_mut();

        // SAFETY: this is used by future, so it will not dangle as future is dropped before context.
        let context_mut = unsafe { context_mut.as_mut().unwrap() };

        // get future, filling box if we can.
        let future = A::get_future(transposer, context_mut, raw_time, arg_passed);

        // SAFETY: forcing the lifetime. This is dropped before the borrowed content so its fine.
        let future: Pin<Box<dyn Future<Output = ()>>> = unsafe { core::mem::transmute(future) };

        Self {
            future,
            arg,
            context,
            wrapped_transposer,
        }
    }

    pub fn reclaim_pending(self) -> A::Stored {
        let Self {
            future,
            arg,
            context,
            wrapped_transposer,
        } = self;

        drop(future);
        drop(context);
        drop(wrapped_transposer);
        arg
    }

    // SAFETY: this must be called after this future resolves.
    pub unsafe fn reclaim_ready(self) -> UpdateResult<T, S, C, A> {
        let Self {
            future,
            arg,
            context,
            wrapped_transposer,
        } = self;

        drop(future);
        let outputs = context.recover_outputs();

        UpdateResult {
            wrapped_transposer,
            outputs,
            arg,
        }
    }
}

impl<T: Transposer, S: StorageFamily, C: UpdateContext<T, S>, A: Arg<T, S>> Future
    for Update<T, S, C, A>
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.future.as_mut().poll(cx)
    }
}

pub struct UpdateResult<T: Transposer, S: StorageFamily, C: UpdateContext<T, S>, A: Arg<T, S>> {
    pub wrapped_transposer: Box<WrappedTransposer<T, S>>,
    pub outputs:            C::Outputs,
    pub arg:                A::Stored,
}
