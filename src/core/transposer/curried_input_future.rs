use super::{
    context::LazyState, transposer::Transposer, transposer_frame::TransposerFrame,
    transposer_function_wrappers::WrappedUpdateResult, UpdateContext,
};
use futures::Future;
use std::{
    marker::PhantomPinned,
    mem::MaybeUninit,
    pin::Pin,
    task::{Context, Poll},
};

pub(super) struct CurriedInputFuture<'a, T: Transposer + 'a> {
    // the curried future; placed first so it is dropped first.
    update_fut: Option<Box<dyn Future<Output = ()> + 'a>>,

    // cx is placed second because it references frame and is referenced by fut.
    update_cx: MaybeUninit<UpdateContext<'a, T>>,

    // curried arguments to the internal future.
    frame: TransposerFrame<T>,
    time: T::Time,
    inputs: Vec<T::Input>,
    state: LazyState<T::InputState>,

    // fut contains references to its curried arguments, so it can't be Unpin.
    _pin: PhantomPinned,
}

// lots of unsafe shenanegans goin on up in here
impl<'a, T: Transposer + 'a> CurriedInputFuture<'a, T> {
    pub fn new(
        frame: TransposerFrame<T>,
        time: T::Time,
        inputs: Vec<T::Input>,
        state: T::InputState,
    ) -> Self {
        Self {
            update_fut: None,
            update_cx: MaybeUninit::uninit(),
            frame,
            time,
            inputs,
            state: LazyState::Ready(state),
            _pin: PhantomPinned,
        }
    }

    pub fn init(self: Pin<&mut Self>) {
        // this is safe because we are adjusting the lifetime
        // to be the lifetime of the pinned struct
        let frame_ref = unsafe {
            let ptr: *mut _ = &mut self.frame;
            ptr.as_mut().unwrap()
        };

        // same with this
        let input_ref = unsafe {
            let ptr: *const _ = &self.inputs;
            ptr.as_ref().unwrap()
        };

        // and with this
        let state_ref = unsafe {
            let ptr: *mut _ = &mut self.state;
            ptr.as_mut().unwrap()
        };

        // create and initialize context
        let cx: UpdateContext<'a, T>;
        cx = UpdateContext::new_input(self.time, &mut frame_ref.expire_handle_factory, state_ref);
        let mut_ref: Pin<&mut Self> = Pin::as_mut(&mut self);
        unsafe {
            Pin::get_unchecked_mut(mut_ref).update_cx = MaybeUninit::new(cx);
        }

        // take ref from newly pinned ref
        let cx_ref = unsafe {
            let ptr: *mut _ = self.update_cx.as_mut_ptr();
            ptr.as_mut().unwrap()
        };

        // initialize update_fut
        let fut = frame_ref.transposer.handle_input(self.time, input_ref, cx_ref);
        let fut = Box::new(fut);
        let mut_ref: Pin<&mut Self> = Pin::as_mut(&mut self);
        unsafe {
            Pin::get_unchecked_mut(mut_ref).update_fut = Some(fut);
        }
    }

    pub fn recover_pinned(self: Pin<&mut Self>) -> (T::Time, Vec<T::Input>, T::InputState) {
        let owned = unsafe { Pin::into_inner_unchecked(self)};
        owned.recover()
    }

    // this does not recover frame because it may have been half-mutated
    pub fn recover(self) -> (T::Time, Vec<T::Input>, T::InputState) {
        let state = match self.state.destroy() {
            Some(s) => s,
            None => unreachable!(),
        };
        (self.time, self.inputs, state)
    }

    pub fn time(&self) -> T::Time {
        self.time
    }
}

impl<'a, T: Transposer + 'a> Future for CurriedInputFuture<'a, T> {
    type Output = WrappedUpdateResult<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let fut: Pin<&mut _> = unsafe {
            if self.get_unchecked_mut().update_fut.is_none() {
                panic!()
            }
            self.map_unchecked_mut(|curried| curried.update_fut.unwrap().as_mut())
        };
        match fut.poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(()) => {
                todo!()
            }
        }
    }
}
