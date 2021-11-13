use core::future::Future;
use core::marker::PhantomPinned;
use core::mem::{ManuallyDrop, MaybeUninit};
use core::pin::Pin;
use core::task::{Context, Poll};

use super::super::frame::Frame;
use super::arg::Arg;
use super::lazy_state::LazyState;
use super::update_context::UpdateContext;
use crate::source::adapters::transpose::engine_time::EngineTime;
use crate::transposer::Transposer;

pub(super) struct FrameUpdatePollable<T: Transposer, A: Arg<T>> {
    // references context, frame.transposer, and args
    future: MaybeUninit<Pin<Box<dyn Future<Output = ()>>>>,

    // references state and frame.internal
    context: MaybeUninit<UpdateContext<T>>,

    frame: Box<Frame<T>>,
    state: LazyState<T::InputState>,
    args:  MaybeUninit<A::Stored>,
    time:  EngineTime<T::Time>,

    // lots of self references. very dangerous.
    _pin: PhantomPinned,
}

impl<T: Transposer, A: Arg<T>> FrameUpdatePollable<T, A> {
    // SAFETY: make sure to call init before doing anything with the new value.
    pub unsafe fn new(frame: Box<Frame<T>>, time: EngineTime<T::Time>) -> Self {
        Self {
            future: MaybeUninit::uninit(),
            context: MaybeUninit::uninit(),
            frame,
            state: LazyState::new(),
            args: MaybeUninit::uninit(),
            time,
            _pin: PhantomPinned,
        }
    }

    pub fn init<'s>(self: Pin<&'s mut Self>, args: A::Passed) -> Result<(), usize> {
        let this = unsafe { self.get_unchecked_mut() };

        // SAFETY: we're storing this in context which is always dropped before frame.
        let internal_ptr: *mut _ = &mut this.frame.metadata;
        let transposer_ptr: *mut _ = &mut this.frame.transposer;
        let state_ptr: *mut _ = &mut this.state;

        // SAFETY: we're storing this in context which is always dropped before state.
        let context = unsafe { UpdateContext::new(this.time.clone(), internal_ptr, state_ptr) };
        this.context = MaybeUninit::new(context);

        // SAFETY: we're storing this in future, which is always dropped before context.
        let context_ptr: *mut _ = unsafe { this.context.assume_init_mut() };
        let context_ref: &'s mut _ = unsafe { context_ptr.as_mut().unwrap() };

        let transposer_ref: &'s mut _ = unsafe { transposer_ptr.as_mut().unwrap() };

        // create our future
        let fut = A::get_fut(
            transposer_ref,
            context_ref,
            this.time.raw_time()?,
            args,
            &mut this.args,
        );

        // SAFETY: transmute the lifetime, because fut will be valid until it is dropped.
        let fut = unsafe { core::mem::transmute(fut) };

        this.future = MaybeUninit::new(fut);

        Ok(())
    }

    pub fn reclaim_pending(self) -> A::Stored {
        let mut this = ManuallyDrop::new(self);

        // SAFETY: future is always initialized.
        unsafe { this.future.assume_init_drop() };

        // SAFETY: context is always initialized.
        unsafe { this.context.assume_init_drop() };

        // SAFETY: because we're forgetting about self we can just sorta go for it.
        let frame: &mut MaybeUninit<Box<Frame<T>>> =
            unsafe { core::mem::transmute(&mut this.frame) };
        // SAFETY: this is initialized cause it's from a non maybeuninit value and transmuted
        unsafe { frame.assume_init_drop() };

        let time: &mut MaybeUninit<EngineTime<T::Time>> =
            unsafe { core::mem::transmute(&mut this.frame) };
        unsafe { time.assume_init_drop() };

        // SAFETY: args is always initialized, and future is the only thing with a reference to it.
        unsafe { this.args.assume_init_read() }
    }

    pub fn reclaim_ready(self) -> (Box<Frame<T>>, Vec<T::Output>, A::Stored) {
        let mut this = ManuallyDrop::new(self);

        // SAFETY: future is always initialized.
        unsafe { this.future.assume_init_drop() };

        // SAFETY: future is always initialized.
        let context = unsafe { this.context.assume_init_read() };
        let outputs = context.recover_outputs();

        // SAFETY: because we're forgetting about self we can just sorta go for it.
        let frame: &mut MaybeUninit<Box<Frame<T>>> =
            unsafe { core::mem::transmute(&mut this.frame) };
        // SAFETY: this is initialized cause it's from a non maybeuninit value and transmuted
        let frame = unsafe { frame.assume_init_read() };

        let time: &mut MaybeUninit<EngineTime<T::Time>> =
            unsafe { core::mem::transmute(&mut this.frame) };
        unsafe { time.assume_init_drop() };

        // SAFETY: args is always initialized, and future is the only thing with a reference to it.
        let args = unsafe { this.args.assume_init_read() };

        (frame, outputs, args)
    }

    pub fn needs_input_state(&self) -> bool {
        self.state.requested()
    }

    pub fn set_input_state(
        self: Pin<&mut Self>,
        state: T::InputState,
    ) -> Result<(), T::InputState> {
        let this = unsafe { self.get_unchecked_mut() };
        this.state.set(state)
    }
}

impl<T: Transposer, A: Arg<T>> Drop for FrameUpdatePollable<T, A> {
    fn drop(&mut self) {
        unsafe {
            self.future.assume_init_drop();
            self.context.assume_init_drop();
            self.args.assume_init_drop();
        }
    }
}

impl<T: Transposer, A: Arg<T>> Future for FrameUpdatePollable<T, A> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: re-pinning
        let this = unsafe { self.get_unchecked_mut() };

        // SAFETY: we always init future immediately after creating it.
        let future = unsafe { this.future.assume_init_mut() };
        let future = future.as_mut();

        future.poll(cx)
    }
}
