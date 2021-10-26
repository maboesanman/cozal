use core::future::Future;
use core::marker::PhantomPinned;
use core::mem::MaybeUninit;
use core::pin::Pin;
use core::task::{Context, Poll};
use std::mem::ManuallyDrop;

use super::super::frame::Frame;
use super::lazy_state::LazyState;
use super::update_context::UpdateContext;
use super::UpdateArgs;
use crate::source::adapters::transpose::engine_time::EngineTime;
use crate::transposer::Transposer;

pub(super) struct FrameUpdatePollable<T: Transposer> {
    // references context and frame.transposer
    future: MaybeUninit<Box<dyn Future<Output = ()>>>,

    // references state and frame.internal
    context: MaybeUninit<UpdateContext<T>>,

    frame:  Frame<T>,
    state:  LazyState<T::InputState>,
    inputs: Option<Vec<T::Input>>,
    time: EngineTime<T::Time>,

    // lots of self references. very dangerous.
    _pin: PhantomPinned,
}

impl<T: Transposer> FrameUpdatePollable<T> {
    // SAFETY: make sure to call init before doing anything with the new value.
    pub unsafe fn new(frame: Frame<T>, time: EngineTime<T::Time>) -> Self {
        Self {
            future: MaybeUninit::uninit(),
            context: MaybeUninit::uninit(),
            frame,
            state: LazyState::new(),

            // this may change during init, if the args are for an input.
            inputs: None,
            time,
            _pin: PhantomPinned,
        }
    }

    pub fn init<'s>(self: Pin<&'s mut Self>, args: UpdateArgs<T>) {
        let this = unsafe { self.get_unchecked_mut() };

        // SAFETY: we're storing this in context which is always dropped before frame.
        let internal_ptr: *mut _ = &mut this.frame.internal;
        let transposer_ptr: *mut _ = &mut this.frame.transposer;
        let state_ptr: *mut _ = &mut this.state;

        // SAFETY: we're storing this in context which is always dropped before state.
        let context = unsafe { UpdateContext::new(
            this.time.clone(),
            internal_ptr,
            state_ptr
        ) };
        this.context = MaybeUninit::new(context);

        // SAFETY: we're storing this in future, which is always dropped before context.
        let context_ptr: *mut _ = unsafe { this.context.assume_init_mut() };
        let context_ref: &'s mut _ = unsafe { context_ptr.as_mut().unwrap() };

        let transposer_ref: &'s mut _ = unsafe { transposer_ptr.as_mut().unwrap() };
        // create our future
        let fut = match args {
            UpdateArgs::Init => transposer_ref.init(context_ref),
            UpdateArgs::Input {
                time,
                inputs,
            } => {
                this.inputs = Some(inputs);

                let inputs_ptr: *const _ = this.inputs.as_ref().unwrap();
                let inputs_ref = unsafe { inputs_ptr.as_ref().unwrap() };

                transposer_ref.handle_input(time, inputs_ref, context_ref)
            },
            UpdateArgs::Scheduled {
                time,
                payload,
            } => transposer_ref.handle_scheduled(time.time, payload, context_ref),
        };
        this.future = MaybeUninit::new(unsafe { std::mem::transmute(fut) });
    }

    pub fn reclaim_pending(mut self) -> Option<Vec<T::Input>> {
        core::mem::take(&mut self.inputs)
    }

    pub fn reclaim_ready(self) -> (Frame<T>, Vec<T::Output>, Option<Vec<T::Input>>) {
        let mut this = ManuallyDrop::new(self);

        // SAFETY: future is always initialized.
        unsafe { this.future.assume_init_drop() };

        // SAFETY: future is always initialized.
        let context = unsafe { this.context.assume_init_read() };
        let outputs = context.recover_outputs();

        // SAFETY: because we're forgetting about self we can just sorta go for it.
        let frame: &mut MaybeUninit<Frame<T>> =
            unsafe { core::mem::transmute(&mut this.frame) };
        // SAFETY: this is initialized cause it's from a non maybeuninit value and transmuted
        let frame = unsafe { frame.assume_init_read() };

        let inputs = core::mem::take(&mut this.inputs);

        (frame, outputs, inputs)
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

impl<T: Transposer> Drop for FrameUpdatePollable<T> {
    fn drop(&mut self) {
        unsafe {
            self.future.assume_init_drop();
            self.context.assume_init_drop();
        }
    }
}

impl<T: Transposer> Future for FrameUpdatePollable<T> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: re-pinning
        let this = unsafe { self.get_unchecked_mut() };

        // SAFETY: we always init future immediately after creating it.
        let future = unsafe { this.future.assume_init_mut() };
        let future = future.as_mut();

        // SAFETY: re-pinning
        let future = unsafe { Pin::new_unchecked(future) };

        future.poll(cx)
    }
}
