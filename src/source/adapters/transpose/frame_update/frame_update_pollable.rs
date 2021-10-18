use core::future::Future;
use core::marker::PhantomPinned;
use core::mem::MaybeUninit;
use core::pin::Pin;
use core::task::{Context, Poll};
use std::mem::ManuallyDrop;

use crate::source::adapters::transpose::lazy_state::LazyState;
use crate::source::adapters::transpose::transposer_frame::TransposerFrame;
use crate::source::adapters::transpose::update_context::UpdateContext;
use crate::transposer::{Transposer};

use super::UpdateType;

pub(super) struct FrameUpdatePollable<'cx, T: Transposer>
    where T::InputState: 'cx
{
    // references context and frame.transposer
    future:  MaybeUninit<Box<dyn Future<Output = ()>>>,

    // references state and frame.internal
    context: MaybeUninit<UpdateContext<'cx, T>>,

    frame: TransposerFrame<T>,
    state: LazyState<T::InputState>,
    inputs: Option<Vec<T::Input>>,

    // lots of self references. very dangerous.
    _pin:    PhantomPinned,
}

impl<'cx, T: Transposer> FrameUpdatePollable<'cx, T>
where T::InputState: 'cx
{
    // SAFETY: make sure to call this back to back with init
    pub unsafe fn new(
        frame: TransposerFrame<T>,
    ) -> Self {
        Self {
            future: MaybeUninit::uninit(),
            context: MaybeUninit::uninit(),
            frame,
            state: LazyState::new(),
            inputs: None,
            _pin: PhantomPinned,
        }
    }

    // SAFETY: make sure to call this back to back with new
    pub unsafe fn init(
        &mut self,
        args: UpdateType<T>,
    ) {

        // SAFETY: we're storing this in context which is always dropped before frame.
        let frame_ptr: *mut _ = &mut self.frame;
        let frame_ref = unsafe { frame_ptr.as_mut().unwrap() };

        let internal_ref = &mut frame_ref.internal;
        let transposer_ref = &mut frame_ref.transposer;

        // SAFETY: we're storing this in context which is always dropped before state.
        let state_ptr: *mut _ = &mut self.state;
        let state_ref = unsafe { state_ptr.as_mut().unwrap() };

        self.context = MaybeUninit::new(UpdateContext::new(internal_ref, state_ref));

        // SAFETY: we're storing this in future, which is always dropped before context.
        let context_ptr: *mut _ = unsafe { self.context.assume_init_mut() };
        let context_ref = unsafe { context_ptr.as_mut().unwrap() };

        // create our future
        let fut = match args {
            UpdateType::Init => transposer_ref.init(context_ref),
            UpdateType::Input { time, inputs } => {
                self.inputs = Some(inputs);

                let inputs_ptr: *const _ = self.inputs.as_ref().unwrap();
                let inputs_ref = unsafe { inputs_ptr.as_ref().unwrap() };

                transposer_ref.handle_input(time, inputs_ref, context_ref)
            },
            UpdateType::Scheduled { time, payload } => transposer_ref.handle_scheduled(time.time, payload, context_ref),
        };
        let fut = Box::new(fut);
        self.future = MaybeUninit::new(fut);
    }

    pub fn reclaim(self) -> (TransposerFrame<T>, Vec<T::Output>) {
        let mut this = ManuallyDrop::new(self);

        // SAFETY: future is always initialized.
        unsafe { this.future.assume_init_drop() };

        // SAFETY: future is always initialized.
        let context = unsafe { this.context.assume_init_read() };
        let outputs = context.recover_outputs();

        // SAFETY: because we're forgetting about self we can just sorta go for it.
        let frame: &mut MaybeUninit<TransposerFrame<T>> = unsafe { core::mem::transmute(&mut this.frame) };
        // SAFETY: this is initialized cause it's from a non maybeuninit value and transmuted
        let frame = unsafe { frame.assume_init_read() };

        (frame, outputs)
    }

    pub fn needs_input_state(&self) -> bool {
        self.state.requested()
    }

    pub fn set_input_state(self: Pin<&mut Self>, state: T::InputState) -> Result<(), T::InputState> {
        let this = unsafe { self.get_unchecked_mut() };
        self.state.set(state)
    }
}

impl<'cx, T: Transposer> Drop for FrameUpdatePollable<'cx, T>{
    fn drop(&mut self) {
        unsafe {
            self.future.assume_init_drop();
            self.context.assume_init_drop();
        }
    }
}

impl<'cx, T: Transposer> Future for FrameUpdatePollable<'cx, T> {
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
