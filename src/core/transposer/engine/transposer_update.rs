use futures::{future::FusedFuture, Future};
use std::{
    marker::PhantomPinned,
    mem::MaybeUninit,
    pin::Pin,
    task::{Context, Poll},
};

use crate::core::Transposer;

use super::{
    engine_context::EngineContext, lazy_state::LazyState, transposer_frame::TransposerFrame,
    update_result::UpdateResult,
};

/// future to initialize a TransposerFrame
///
/// the initialization happens AS A SIDE EFFECT OF THIS.
pub struct TransposerUpdate<'f, T: Transposer>(TransposerUpdateState<'f, T>)
where
    T::Scheduled: Clone;

enum TransposerUpdateState<'f, T: Transposer> {
    // Unpollable variants hold the references until we can create the pollable future
    UnpollableInit {
        frame_ref: &'f mut TransposerFrame<'f, T>,
        state_ref: &'f mut LazyState<T::InputState>,
    },
    UnpollableInput {
        frame_ref: &'f mut TransposerFrame<'f, T>,
        state_ref: &'f mut LazyState<T::InputState>,
        time: T::Time,
        inputs: &'f [T::Input],
    },
    UnpollableScheduled {
        frame_ref: &'f mut TransposerFrame<'f, T>,
        state_ref: &'f mut LazyState<T::InputState>,
        time: T::Time,
        payload: T::Scheduled,
    },
    // this can be split into separate PollableInit/Input/Scheduled someday if named existentials become a thing.
    Pollable(TransposerUpdatePollableState<'f, T>),
    Terminated,
    Poisioned,
}

struct TransposerUpdatePollableState<'f, T: Transposer> {
    // this is dyn until named existentials become a thing, then it would be T::init::Future or however the syntax ends up.
    future: MaybeUninit<Box<dyn Future<Output = ()> + 'f>>,

    // cx is placed second because it is referenced by fut.
    context: EngineContext<'f, T>,

    // future contains a reference to context.
    _pin: PhantomPinned,
}

impl<'f, T: Transposer> Drop for TransposerUpdatePollableState<'f, T> {
    fn drop(&mut self) {
        // SAFETY: this is always init because we create the Pollable variant and immediately initialize it
        unsafe {
            self.future.assume_init_drop();
        }
    }
}

impl<'f, T: Transposer> TransposerUpdatePollableState<'f, T> {
    pub fn recover_cx(mut self) -> EngineContext<'f, T> {
        let ret = unsafe { std::ptr::read(&mut self.context) };
        unsafe {
            self.future.assume_init_drop();
        }
        std::mem::forget(self);
        ret
    }
}

impl<'f, T: Transposer> TransposerUpdate<'f, T>
where
    T::Scheduled: Clone,
{
    #[allow(unused)]
    pub fn new_init(
        frame_ref: &'f mut TransposerFrame<'f, T>,
        state_ref: &'f mut LazyState<T::InputState>,
    ) -> Self {
        Self(TransposerUpdateState::UnpollableInit {
            frame_ref,
            state_ref,
        })
    }

    #[allow(unused)]
    pub fn new_input(
        frame_ref: &'f mut TransposerFrame<'f, T>,
        state_ref: &'f mut LazyState<T::InputState>,
        time: T::Time,
        inputs: &'f [T::Input],
    ) -> Self {
        Self(TransposerUpdateState::UnpollableInput {
            frame_ref,
            state_ref,
            time,
            inputs,
        })
    }

    #[allow(unused)]
    pub fn new_scheduled(
        frame_ref: &'f mut TransposerFrame<'f, T>,
        state_ref: &'f mut LazyState<T::InputState>,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Self {
        Self(TransposerUpdateState::UnpollableScheduled {
            frame_ref,
            state_ref,
            time,
            payload,
        })
    }
}

impl<'f, T: Transposer> Future for TransposerUpdate<'f, T> {
    type Output = UpdateResult<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        // SAFETY the pin is only relavent for the pollable variant, which we don't move around.
        let this = unsafe { self.get_unchecked_mut() };
        let inner = &mut this.0;
        'poll: loop {
            match inner {
                TransposerUpdateState::UnpollableInit { .. } => {
                    // get vars ready to take out of self.
                    let mut transposer_ref: MaybeUninit<&'f mut T> = MaybeUninit::uninit();

                    // take out of self and replace with a Pollable with uninit future.
                    take_mut::take_or_recover(
                        inner,
                        || TransposerUpdateState::Poisioned,
                        |inner_owned| {
                            if let TransposerUpdateState::UnpollableInit {
                                frame_ref,
                                state_ref,
                            } = inner_owned
                            {
                                let context: EngineContext<'f, T>;
                                context = EngineContext::new(&mut frame_ref.internal, state_ref);
                                transposer_ref = MaybeUninit::new(&mut frame_ref.transposer);

                                TransposerUpdateState::Pollable(TransposerUpdatePollableState {
                                    future: MaybeUninit::uninit(),
                                    context,
                                    _pin: PhantomPinned,
                                })
                            } else {
                                unreachable!()
                            }
                        },
                    );

                    // init out uninit future.
                    if let TransposerUpdateState::Pollable(TransposerUpdatePollableState {
                        future,
                        context,
                        _pin,
                    }) = inner
                    {
                        // SAFETY: if we are in pollable state, the take_or_recover call did not panic and these have been initialized.
                        let transposer_ref = unsafe { transposer_ref.assume_init() };

                        // take ref from newly pinned ref
                        let ptr: *mut _ = context;

                        // SAFETY: this is
                        let cx_ref = unsafe { ptr.as_mut().unwrap() };

                        // create our future
                        let fut = transposer_ref.init(cx_ref);
                        let fut = Box::new(fut);
                        *future = MaybeUninit::new(fut);
                    } else {
                        unreachable!()
                    }
                }
                TransposerUpdateState::UnpollableInput { .. } => {
                    // get vars ready to take out of self.
                    let mut transposer_ref: MaybeUninit<&'f mut T> = MaybeUninit::uninit();
                    let mut time_val: MaybeUninit<T::Time> = MaybeUninit::uninit();
                    let mut inputs_ref: MaybeUninit<&'f [T::Input]> = MaybeUninit::uninit();

                    // take out of self and replace with a Pollable with uninit future.
                    take_mut::take_or_recover(
                        inner,
                        || TransposerUpdateState::Poisioned,
                        |inner_owned| {
                            if let TransposerUpdateState::UnpollableInput {
                                frame_ref,
                                state_ref,
                                time,
                                inputs,
                            } = inner_owned
                            {
                                let context: EngineContext<'f, T>;
                                context = EngineContext::new(&mut frame_ref.internal, state_ref);
                                transposer_ref = MaybeUninit::new(&mut frame_ref.transposer);
                                time_val = MaybeUninit::new(time);
                                inputs_ref = MaybeUninit::new(inputs);

                                TransposerUpdateState::Pollable(TransposerUpdatePollableState {
                                    future: MaybeUninit::uninit(),
                                    context,
                                    _pin: PhantomPinned,
                                })
                            } else {
                                unreachable!()
                            }
                        },
                    );

                    // init out uninit future.
                    if let TransposerUpdateState::Pollable(TransposerUpdatePollableState {
                        future,
                        context,
                        _pin,
                    }) = inner
                    {
                        // SAFETY: if we are in pollable state, the take_or_recover call did not panic and these have been initialized.
                        let transposer_ref = unsafe { transposer_ref.assume_init() };
                        let time_val = unsafe { time_val.assume_init() };
                        let inputs_ref = unsafe { inputs_ref.assume_init() };

                        // take ref from newly pinned ref
                        let ptr: *mut _ = context;

                        // SAFETY: this is
                        let cx_ref = unsafe { ptr.as_mut().unwrap() };

                        // create our future
                        let fut = transposer_ref.handle_input(time_val, inputs_ref, cx_ref);
                        let fut = Box::new(fut);
                        *future = MaybeUninit::new(fut);
                    } else {
                        unreachable!()
                    }
                }
                TransposerUpdateState::UnpollableScheduled { .. } => {
                    // get vars ready to take out of self.
                    let mut transposer_ref: MaybeUninit<&'f mut T> = MaybeUninit::uninit();
                    let mut time_val: MaybeUninit<T::Time> = MaybeUninit::uninit();
                    let mut payload_val: MaybeUninit<T::Scheduled> = MaybeUninit::uninit();

                    // take out of self and replace with a Pollable with uninit future.
                    take_mut::take_or_recover(
                        inner,
                        || TransposerUpdateState::Poisioned,
                        |inner_owned| {
                            if let TransposerUpdateState::UnpollableScheduled {
                                frame_ref,
                                state_ref,
                                time,
                                payload,
                            } = inner_owned
                            {
                                let context: EngineContext<'f, T>;
                                context = EngineContext::new(&mut frame_ref.internal, state_ref);
                                transposer_ref = MaybeUninit::new(&mut frame_ref.transposer);
                                time_val = MaybeUninit::new(time);
                                payload_val = MaybeUninit::new(payload);

                                TransposerUpdateState::Pollable(TransposerUpdatePollableState {
                                    future: MaybeUninit::uninit(),
                                    context,
                                    _pin: PhantomPinned,
                                })
                            } else {
                                unreachable!()
                            }
                        },
                    );

                    // init out uninit future.
                    if let TransposerUpdateState::Pollable(TransposerUpdatePollableState {
                        future,
                        context,
                        _pin,
                    }) = inner
                    {
                        // SAFETY: if we are in pollable state, the take_or_recover call did not panic and these have been initialized.
                        let transposer_ref = unsafe { transposer_ref.assume_init() };
                        let time_val = unsafe { time_val.assume_init() };
                        let payload_val = unsafe { payload_val.assume_init() };

                        // take ref from newly pinned ref
                        let ptr: *mut _ = context;

                        // SAFETY: this is
                        let cx_ref = unsafe { ptr.as_mut().unwrap() };

                        // create our future
                        let fut = transposer_ref.handle_scheduled(time_val, payload_val, cx_ref);
                        let fut = Box::new(fut);
                        *future = MaybeUninit::new(fut);
                    } else {
                        unreachable!()
                    }
                }
                TransposerUpdateState::Pollable(TransposerUpdatePollableState {
                    future, ..
                }) => {
                    // SAFETY: structural pinning. we don't move the future if we don't move self.
                    let future = unsafe { Pin::new_unchecked(future.assume_init_mut().as_mut()) };

                    // pass through the poll
                    break 'poll match future.poll(cx) {
                        Poll::Ready(()) => {
                            if let TransposerUpdateState::Pollable(state) =
                                std::mem::replace(inner, TransposerUpdateState::Terminated)
                            {
                                Poll::Ready(state.recover_cx().into())
                            } else {
                                unreachable!()
                            }
                        }
                        Poll::Pending => Poll::Pending,
                    };
                }
                TransposerUpdateState::Terminated => unreachable!(),
                TransposerUpdateState::Poisioned => todo!(),
            };
        }
    }
}

impl<'a, T: Transposer> FusedFuture for TransposerUpdate<'a, T> {
    fn is_terminated(&self) -> bool {
        if let Self(TransposerUpdateState::Terminated) = self {
            true
        } else {
            false
        }
    }
}
