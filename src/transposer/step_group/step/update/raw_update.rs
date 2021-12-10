use core::future::Future;
use core::marker::PhantomPinned;
use core::mem::{ManuallyDrop, MaybeUninit};
use core::pin::Pin;
use core::task::{Context, Poll};

use super::{Arg, StepTime, UpdateContext, WrappedTransposer};
use crate::transposer::step_group::lazy_state::LazyState;
use crate::transposer::Transposer;
use crate::util::drop_mut::{drop_mut_leave_uninit, take_mut_leave_uninit};

pub struct RawUpdate<T: Transposer, C: UpdateContext<T>, A: Arg<T>> {
    // references context, frame.transposer, and args
    future: MaybeUninit<Pin<Box<dyn Future<Output = ()>>>>,

    // references state and frame.internal
    context: MaybeUninit<C>,

    frame: Box<WrappedTransposer<T>>,
    state: *const LazyState<T::InputState>,
    time:  StepTime<T::Time>,
    args:  MaybeUninit<A::Stored>,

    // lots of self references. very dangerous.
    _pin: PhantomPinned,
}

impl<T: Transposer, C: UpdateContext<T>, A: Arg<T>> RawUpdate<T, C, A> {
    // SAFETY: make sure to call init before doing anything with the new value, including dropping it.
    pub unsafe fn new(
        mut frame: Box<WrappedTransposer<T>>,
        time: StepTime<T::Time>,
        state: *const LazyState<T::InputState>,
    ) -> Self {
        frame.metadata.last_updated = time.raw_time();
        Self {
            future: MaybeUninit::uninit(),
            context: MaybeUninit::uninit(),
            frame,
            state,
            args: MaybeUninit::uninit(),
            time,
            _pin: PhantomPinned,
        }
    }

    pub fn init(self: Pin<&mut Self>, args: A::Passed) -> Result<(), usize> {
        let this = unsafe { self.get_unchecked_mut() };

        let context = &mut this.context;
        let transposer = &mut this.frame.transposer;
        let metadata = &mut this.frame.metadata;
        let input_state = this.state;

        // SAFETY: we're storing this in context which is always dropped before metadata and input_state.
        *context = MaybeUninit::new(unsafe { C::new(this.time.clone(), metadata, input_state) });

        // SAFETY: we just assigned to this.
        let context = unsafe { context.assume_init_mut() };

        // create our future
        let fut = A::get_fut(
            transposer,
            context,
            this.time.raw_time(),
            args,
            &mut this.args,
        );

        // SAFETY: transmute the lifetime, future is dropped before transposer and context.
        let fut = unsafe { core::mem::transmute(fut) };

        this.future = MaybeUninit::new(fut);

        Ok(())
    }

    pub fn reclaim_pending(self) -> A::Stored {
        let mut this = ManuallyDrop::new(self);

        // SAFETY: everything is init before this.
        // we manually drop or recover data from each field in drop order.
        unsafe {
            this.future.assume_init_drop();
            this.context.assume_init_drop();
            drop_mut_leave_uninit(&mut this.frame);
            drop_mut_leave_uninit(&mut this.state);
            drop_mut_leave_uninit(&mut this.time);
            this.args.assume_init_read()
        }
    }

    // SAFETY: this must be called after this future resolves.
    pub unsafe fn reclaim_ready(self) -> (Box<WrappedTransposer<T>>, C::Outputs, A::Stored) {
        let mut this = ManuallyDrop::new(self);

        // SAFETY: everything is init before this.
        // we manually drop or recover data from each field in drop order.
        unsafe {
            this.future.assume_init_drop();
            let context = this.context.assume_init_read();
            let outputs = context.recover_outputs();
            let frame = take_mut_leave_uninit(&mut this.frame);
            drop_mut_leave_uninit(&mut this.state);
            drop_mut_leave_uninit(&mut this.time);
            let args = this.args.assume_init_read();
            (frame, outputs, args)
        }
    }

    pub fn needs_input_state(&self) -> bool {
        self.get_state().requested()
    }

    pub fn set_input_state(&self, state: T::InputState) -> Result<(), T::InputState> {
        self.get_state().set(state)
    }

    fn get_state(&self) -> &LazyState<T::InputState> {
        unsafe { self.state.as_ref().unwrap() }
    }
}

impl<T: Transposer, C: UpdateContext<T>, A: Arg<T>> Drop for RawUpdate<T, C, A> {
    fn drop(&mut self) {
        // SAFETY: everything is init before this.
        unsafe {
            self.future.assume_init_drop();
            self.context.assume_init_drop();
            self.args.assume_init_drop();
        }
    }
}

impl<T: Transposer, C: UpdateContext<T>, A: Arg<T>> Future for RawUpdate<T, C, A> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: we always init future immediately after creating it.
        let future = unsafe {
            let this = self.get_unchecked_mut();
            this.future.assume_init_mut()
        };

        future.as_mut().poll(cx)
    }
}
