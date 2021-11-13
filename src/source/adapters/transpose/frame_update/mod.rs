use core::future::Future;
use core::mem::MaybeUninit;
use core::pin::Pin;
use core::task::{Context, Poll};

use futures_core::FusedFuture;

use self::arg::Arg;
use self::frame_update_pollable::FrameUpdatePollable;
pub use self::update_result::UpdateResult;
use super::engine_time::EngineTime;
use super::frame::Frame;
use crate::transposer::Transposer;
pub(crate) mod arg;
mod frame_update_pollable;
mod lazy_state;
mod update_context;
mod update_result;

/// future to initialize a TransposerFrame
///
/// this type owns a frame, and has many self references.
pub struct FrameUpdate<T: Transposer, A: Arg<T>> {
    inner: FrameUpdateInner<T, A>,
}

enum FrameUpdateInner<T: Transposer, A: Arg<T>> {
    // Unpollable variants hold the references until we can create the pollable future
    Unpollable(FrameUpdateUnpollable<T, A>),
    Pollable(FrameUpdatePollable<T, A>),
    Terminated,
}

struct FrameUpdateUnpollable<T: Transposer, A: Arg<T>> {
    frame: Box<Frame<T>>,
    args:  A::Passed,
    time:  EngineTime<T::Time>,
}

impl<T: Transposer, A: Arg<T>> FrameUpdate<T, A>
where
    T::Scheduled: Clone,
{
    pub fn new(mut frame: Box<Frame<T>>, arg: A::Stored, time: EngineTime<T::Time>) -> Self {
        debug_assert!(time.is_init());

        Self {
            inner: FrameUpdateInner::Unpollable(FrameUpdateUnpollable {
                args: A::get_arg(&mut frame, arg, &time),
                frame,
                time,
            }),
        }
    }

    pub fn reclaim(self: Pin<&mut Self>) -> A::Stored {
        let mut result = None;

        let this = unsafe { self.get_unchecked_mut() };

        take_mut::take(&mut this.inner, |inner| match inner {
            FrameUpdateInner::Unpollable(u) => {
                result = Some(A::get_stored(u.args));
                FrameUpdateInner::Terminated
            },
            FrameUpdateInner::Pollable(p) => {
                result = Some(p.reclaim_pending());
                FrameUpdateInner::Terminated
            },
            other => other,
        });

        result.unwrap()
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

impl<T: Transposer, A: Arg<T>> Future for FrameUpdate<T, A> {
    type Output = UpdateResult<T, A>;

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

impl<T: Transposer, A: Arg<T>> FusedFuture for FrameUpdate<T, A> {
    fn is_terminated(&self) -> bool {
        matches!(self.inner, FrameUpdateInner::Terminated)
    }
}
