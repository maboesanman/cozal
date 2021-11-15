use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use futures_core::FusedFuture;
use pin_project::pin_project;

use super::{Arg, EngineTime, Frame, FrameUpdatePollable, UpdateContext, UpdateResult};
use crate::transposer::Transposer;
use crate::util::take_mut;

/// future to initialize a TransposerFrame
///
/// this type owns a frame, and has many self references.
#[pin_project]
pub struct FrameUpdate<T: Transposer, C: UpdateContext<T>, A: Arg<T>> {
    #[pin]
    inner: FrameUpdateInner<T, C, A>,
}

#[pin_project(project=FrameUpdateInnerProject)]
enum FrameUpdateInner<T: Transposer, C: UpdateContext<T>, A: Arg<T>> {
    // Unpollable variants hold the references until we can create the pollable future
    Unpollable(FrameUpdateUnpollable<T, A>),
    Pollable(#[pin] FrameUpdatePollable<T, C, A>),
    Terminated,
}

struct FrameUpdateUnpollable<T: Transposer, A: Arg<T>> {
    frame: Box<Frame<T>>,
    args:  A::Passed,
    time:  EngineTime<T::Time>,
}

impl<T: Transposer, C: UpdateContext<T>, A: Arg<T>> FrameUpdate<T, C, A>
where
    T::Scheduled: Clone,
{
    pub fn new(mut frame: Box<Frame<T>>, arg: A::Stored, time: EngineTime<T::Time>) -> Self {
        Self {
            inner: FrameUpdateInner::Unpollable(FrameUpdateUnpollable {
                args: A::get_arg(&mut frame, arg),
                frame,
                time,
            }),
        }
    }

    pub fn reclaim(self: Pin<&mut Self>) -> Result<A::Stored, ()> {
        // SAFETY: we are discarding the pollable variant, so the pin invariants on pollable are upheld.
        let this = unsafe { self.get_unchecked_mut() };

        take_mut::take_and_return_or_recover(
            &mut this.inner,
            || FrameUpdateInner::Terminated,
            |inner| match inner {
                FrameUpdateInner::Unpollable(u) => {
                    (FrameUpdateInner::Terminated, Ok(A::get_stored(u.args)))
                },
                FrameUpdateInner::Pollable(p) => {
                    (FrameUpdateInner::Terminated, Ok(p.reclaim_pending()))
                },
                other => (other, Err(())),
            },
        )
    }

    pub fn needs_input_state(&self) -> Result<bool, ()> {
        match &self.inner {
            FrameUpdateInner::Unpollable(_) => Ok(false),
            FrameUpdateInner::Pollable(p) => Ok(p.needs_input_state()),
            FrameUpdateInner::Terminated => Err(()),
        }
    }

    pub fn set_input_state(
        self: Pin<&mut Self>,
        state: T::InputState,
    ) -> Result<(), T::InputState> {
        match self.project().inner.project() {
            FrameUpdateInnerProject::Pollable(p) => p.set_input_state(state),
            _ => Err(state),
        }
    }
}

impl<T: Transposer, C: UpdateContext<T>, A: Arg<T>> Future for FrameUpdate<T, C, A> {
    type Output = UpdateResult<T, C, A>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut this = self.project();
        'poll: loop {
            // SAFETY the pin is only relavent for the pollable variant, which we don't move around.
            let inner = unsafe { this.inner.as_mut().get_unchecked_mut() };
            let _ = match inner {
                FrameUpdateInner::Unpollable(_) => {
                    // take out of self and replace with a Pollable with uninit future.
                    if let FrameUpdateInner::Unpollable(FrameUpdateUnpollable {
                        frame,
                        args,
                        time,
                    }) = core::mem::replace(inner, FrameUpdateInner::Terminated)
                    {
                        // SAFETY: we init this immediately after placing it.
                        *inner = FrameUpdateInner::Pollable(unsafe {
                            FrameUpdatePollable::new(frame, time)
                        });

                        // init out uninit context and future.
                        if let FrameUpdateInnerProject::Pollable(pollable) =
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
                FrameUpdateInner::Pollable(pollable) => {
                    // SAFETY: structural pinning. we don't move the future if we don't move self.
                    let pollable = unsafe { Pin::new_unchecked(pollable) };

                    // pass through the poll
                    break 'poll match pollable.poll(cx) {
                        Poll::Ready(()) => {
                            if let FrameUpdateInner::Pollable(pollable) =
                                core::mem::replace(inner, FrameUpdateInner::Terminated)
                            {
                                let (frame, outputs, arg) = pollable.reclaim_ready();

                                Poll::Ready(UpdateResult {
                                    frame,
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
                FrameUpdateInner::Terminated => break 'poll Poll::Pending,
            };
        }
    }
}

impl<T: Transposer, C: UpdateContext<T>, A: Arg<T>> FusedFuture for FrameUpdate<T, C, A> {
    fn is_terminated(&self) -> bool {
        matches!(self.inner, FrameUpdateInner::Terminated)
    }
}
