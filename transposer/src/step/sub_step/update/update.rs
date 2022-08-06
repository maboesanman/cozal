use core::future::Future;
use core::pin::Pin;
use core::ptr::NonNull;
use core::task::{Context, Poll};

use super::{Arg, SubStepTime, UpdateContext, WrappedTransposer};
use crate::schedule_storage::{StorageFamily, TransposerPointer};
use crate::step::lazy_state::LazyState;
use crate::Transposer;

pub struct Update<T: Transposer, S: StorageFamily, C: UpdateContext<T, S>, A: Arg<T, S>> {
    inner: Option<UpdateInner<T, S, C>>,

    arg: A::Stored,
}

struct UpdateInner<T: Transposer, S: StorageFamily, C: UpdateContext<T, S>> {
    // references context, wrapped_transposer.transposer, and args
    future: Pin<Box<dyn Future<Output = ()>>>,

    // references state and wrapped_transposer.internal
    context: Box<C>,

    // may be partially modified.
    // treated as borrowed, split between future and context.
    wrapped_transposer: S::Transposer<WrappedTransposer<T, S>>,
}

impl<T: Transposer, S: StorageFamily, C: UpdateContext<T, S>, A: Arg<T, S>> Update<T, S, C, A> {
    pub fn new(
        mut wrapped_transposer: S::Transposer<WrappedTransposer<T, S>>,
        mut arg: A::Stored,
        time: SubStepTime<T::Time>,
        input_state: S::LazyState<LazyState<T::InputState>>,
    ) -> Self {
        // update 'current time'
        let raw_time = time.raw_time();
        let wrapped_transposer_mut = wrapped_transposer.mutate();
        wrapped_transposer_mut.metadata.last_updated = raw_time;

        // // upgrade arg from borrowed to passed (pulling the event from the schedule if scheduled).
        let arg_passed = A::get_passed(wrapped_transposer_mut, &mut arg);

        // split borrow. one goes to fut, one to context.
        let transposer = &mut wrapped_transposer_mut.transposer;
        let metadata = &mut wrapped_transposer_mut.metadata;

        // SAFETY: metadata is stable, and contained in wrapped_transposer
        let context = unsafe { C::new(time, metadata.into(), input_state) };
        let mut context = Box::new(context);
        let mut context_mut: NonNull<_> = context.as_mut().into();

        // SAFETY: this is owned by future, so it will not dangle as future is dropped before context.
        let context_mut = unsafe { context_mut.as_mut() };

        // get future, filling box if we can.
        let future = A::get_future(transposer, context_mut, raw_time, arg_passed);

        // SAFETY: forcing the lifetime. This is dropped before the borrowed content so its fine.
        let future: Pin<Box<dyn Future<Output = ()>>> = future;
        let future: Pin<Box<dyn Future<Output = ()>>> = unsafe { core::mem::transmute(future) };

        Self {
            inner: Some(UpdateInner {
                future,
                context,
                wrapped_transposer,
            }),
            arg,
        }
    }

    pub fn reclaim_pending(self) -> A::Stored {
        let Self {
            inner,
            arg,
        } = self;

        drop(inner);
        arg
    }

    pub fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> UpdatePoll<T, S> {
        let inner = match &mut self.inner {
            Some(inner) => inner,
            None => return UpdatePoll::Pending,
        };
        match inner.future.as_mut().poll(cx) {
            Poll::Pending => match inner.context.recover_output() {
                Some(o) => UpdatePoll::Event(o),
                None => UpdatePoll::Pending,
            },
            Poll::Ready(()) => {
                let UpdateInner {
                    future,
                    context,
                    wrapped_transposer,
                } = core::mem::take(&mut self.inner).unwrap();

                drop(future);
                drop(context);
                UpdatePoll::Ready(wrapped_transposer)
            },
        }
    }
}

pub enum UpdatePoll<T: Transposer, S: StorageFamily> {
    Pending,
    Event(T::Output),
    Ready(S::Transposer<WrappedTransposer<T, S>>),
}
