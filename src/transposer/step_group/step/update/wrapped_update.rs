use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use futures_core::FusedFuture;
use pin_project::pin_project;

use super::{Arg, RawUpdate, StepTime, UpdateContext, UpdateResult, WrappedTransposer};
use crate::transposer::step_group::lazy_state::LazyState;
use crate::transposer::Transposer;
use crate::util::take_mut;

/// future to initialize a TransposerFrame
#[pin_project]
pub struct WrappedUpdate<T: Transposer, C: UpdateContext<T>, A: Arg<T>> {
    #[pin]
    inner: WrappedUpdateInner<T, C, A>,
}

#[pin_project(project=FrameUpdateInnerProject)]
enum WrappedUpdateInner<T: Transposer, C: UpdateContext<T>, A: Arg<T>> {
    // Unpollable variants hold the references until we can create the pollable future
    Waiting(UpdateData<T, A>),
    Active(#[pin] RawUpdate<T, C, A>),
    Terminated,
}

struct UpdateData<T: Transposer, A: Arg<T>> {
    frame: Box<WrappedTransposer<T>>,
    args:  A::Passed,
    time:  StepTime<T::Time>,
    state: *const LazyState<T::InputState>,
}

impl<T: Transposer, C: UpdateContext<T>, A: Arg<T>> WrappedUpdate<T, C, A>
where
    T::Scheduled: Clone,
{
    pub fn new(
        mut frame: Box<WrappedTransposer<T>>,
        arg: A::Stored,
        time: StepTime<T::Time>,
        state: *const LazyState<T::InputState>,
    ) -> Self {
        Self {
            inner: WrappedUpdateInner::Waiting(UpdateData {
                args: A::get_arg(&mut frame, arg),
                frame,
                state,
                time,
            }),
        }
    }

    pub fn reclaim(self: Pin<&mut Self>) -> Result<A::Stored, ()> {
        // SAFETY: we are discarding the pollable variant, so the pin invariants on pollable are upheld.
        let this = unsafe { self.get_unchecked_mut() };

        take_mut::take_and_return_or_recover(
            &mut this.inner,
            || WrappedUpdateInner::Terminated,
            |inner| match inner {
                WrappedUpdateInner::Waiting(u) => {
                    (WrappedUpdateInner::Terminated, Ok(A::get_stored(u.args)))
                },
                WrappedUpdateInner::Active(p) => {
                    (WrappedUpdateInner::Terminated, Ok(p.reclaim_pending()))
                },
                other => (other, Err(())),
            },
        )
    }

    pub fn needs_input_state(&self) -> Result<bool, ()> {
        match &self.inner {
            WrappedUpdateInner::Waiting(_) => Ok(false),
            WrappedUpdateInner::Active(p) => Ok(p.needs_input_state()),
            WrappedUpdateInner::Terminated => Err(()),
        }
    }

    pub fn set_input_state(
        self: Pin<&mut Self>,
        state: T::InputState,
    ) -> Result<(), T::InputState> {
        match self.project().inner.project() {
            FrameUpdateInnerProject::Active(p) => p.set_input_state(state),
            _ => Err(state),
        }
    }
}

impl<T: Transposer, C: UpdateContext<T>, A: Arg<T>> Future for WrappedUpdate<T, C, A> {
    type Output = UpdateResult<T, C, A>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut this = self.project();
        'poll: loop {
            // SAFETY the pin is only relavent for the pollable variant, which we don't move around.
            let inner = unsafe { this.inner.as_mut().get_unchecked_mut() };
            let _ = match inner {
                WrappedUpdateInner::Waiting(_) => {
                    // take out of self and replace with a Pollable with uninit future.
                    if let WrappedUpdateInner::Waiting(UpdateData {
                        frame,
                        args,
                        time,
                        state,
                    }) = core::mem::replace(inner, WrappedUpdateInner::Terminated)
                    {
                        // SAFETY: we init this immediately after placing it.
                        *inner = WrappedUpdateInner::Active(unsafe {
                            RawUpdate::new(frame, time, state)
                        });

                        // init out uninit context and future.
                        if let FrameUpdateInnerProject::Active(pollable) =
                            this.inner.as_mut().project()
                        {
                            pollable.init(args)
                        } else {
                            unreachable!()
                        }
                    } else {
                        unreachable!()
                    }
                },
                WrappedUpdateInner::Active(pollable) => {
                    // SAFETY: structural pinning. we don't move the future if we don't move self.
                    let pollable = unsafe { Pin::new_unchecked(pollable) };

                    // pass through the poll
                    break 'poll match pollable.poll(cx) {
                        Poll::Ready(()) => {
                            if let WrappedUpdateInner::Active(pollable) =
                                core::mem::replace(inner, WrappedUpdateInner::Terminated)
                            {
                                // SAFETY: we got the ready value, so this can be called.
                                let (frame, outputs, arg) = unsafe { pollable.reclaim_ready() };

                                Poll::Ready(UpdateResult {
                                    wrapped_transposer: frame,
                                    outputs,
                                    arg,
                                })
                            } else {
                                unreachable!()
                            }
                        },
                        Poll::Pending => Poll::Pending,
                    }
                },
                WrappedUpdateInner::Terminated => break 'poll Poll::Pending,
            };
        }
    }
}

impl<T: Transposer, C: UpdateContext<T>, A: Arg<T>> FusedFuture for WrappedUpdate<T, C, A> {
    fn is_terminated(&self) -> bool {
        matches!(self.inner, WrappedUpdateInner::Terminated)
    }
}
