use core::future::Future;
use core::mem::MaybeUninit;
use core::pin::Pin;
use core::task::{Context, Poll};

use futures_core::FusedFuture;

use super::arg::Arg;
use super::engine_time::EngineTime;
use super::frame::Frame;
use super::frame_update_pollable::FrameUpdatePollable;
use super::update_context::UpdateContext;
use super::update_result::UpdateResult;
use crate::transposer::Transposer;

/// future to initialize a TransposerFrame
///
/// this type owns a frame, and has many self references.
pub struct FrameUpdate<T: Transposer, C: UpdateContext<T>, A: Arg<T>> {
    inner: FrameUpdateInner<T, C, A>,
}

enum FrameUpdateInner<T: Transposer, C: UpdateContext<T>, A: Arg<T>> {
    // Unpollable variants hold the references until we can create the pollable future
    Unpollable(FrameUpdateUnpollable<T, A>),
    Pollable(FrameUpdatePollable<T, C, A>),
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
        let mut result = Err(());

        let this = unsafe { self.get_unchecked_mut() };

        take_mut::take(&mut this.inner, |inner| match inner {
            FrameUpdateInner::Unpollable(u) => {
                result = Ok(A::get_stored(u.args));
                FrameUpdateInner::Terminated
            },
            FrameUpdateInner::Pollable(p) => {
                result = Ok(p.reclaim_pending());
                FrameUpdateInner::Terminated
            },
            other => other,
        });

        result
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
        let this = unsafe { self.get_unchecked_mut() };
        match &mut this.inner {
            FrameUpdateInner::Pollable(p) => {
                let p = unsafe { Pin::new_unchecked(p) };
                p.set_input_state(state)
            },
            _ => Err(state),
        }
    }
}

impl<T: Transposer, C: UpdateContext<T>, A: Arg<T>> Future for FrameUpdate<T, C, A> {
    type Output = UpdateResult<T, C, A>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        // SAFETY the pin is only relavent for the pollable variant, which we don't move around.
        let this = unsafe { self.get_unchecked_mut() };
        let inner = &mut this.inner;
        'poll: loop {
            let _ = match inner {
                FrameUpdateInner::Unpollable(_) => {
                    // get vars ready to take out of self.
                    let mut args: MaybeUninit<A::Passed> = MaybeUninit::uninit();

                    // take out of self and replace with a Pollable with uninit future.
                    take_mut::take(inner, |inner_owned| {
                        if let FrameUpdateInner::Unpollable(FrameUpdateUnpollable {
                            frame,
                            args: args_temp,
                            time,
                        }) = inner_owned
                        {
                            args = MaybeUninit::new(args_temp);

                            // SAFETY: we are calling init in just a few lines
                            FrameUpdateInner::Pollable(unsafe {
                                FrameUpdatePollable::new(frame, time)
                            })
                        } else {
                            unreachable!()
                        }
                    });

                    // init out uninit context and future.
                    if let FrameUpdateInner::Pollable(pollable) = inner {
                        // SAFETY: these must have been assigned if we are now pollable.
                        let args = unsafe { args.assume_init() };
                        let pollable = unsafe { Pin::new_unchecked(pollable) };

                        // SAFETY: new must have been called if we are now pollable
                        pollable.init(args)
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
