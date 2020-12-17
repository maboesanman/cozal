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

        if self.update_fut.is_some() {
            panic!()
        }

        let this = unsafe { self.get_unchecked_mut() };

        let frame_ref = unsafe {
            let ptr: *mut _ = &mut this.frame;
            ptr.as_mut().unwrap()
        };

        // same with this
        let input_ref = unsafe {
            let ptr: *const _ = &this.inputs;
            ptr.as_ref().unwrap()
        };

        // and with this
        let state_ref = unsafe {
            let ptr: *mut _ = &mut this.state;
            ptr.as_mut().unwrap()
        };

        // create and initialize context
        let cx: UpdateContext<'a, T>;
        cx = UpdateContext::new_input(this.time, &mut frame_ref.expire_handle_factory, state_ref);
        this.update_cx = MaybeUninit::new(cx);

        // take ref from newly pinned ref
        let cx_ref = unsafe {
            let ptr: *mut _ = this.update_cx.as_mut_ptr();
            ptr.as_mut().unwrap()
        };

        // initialize update_fut
        let fut = frame_ref.transposer.handle_input(this.time, input_ref, cx_ref);
        let fut = Box::new(fut);
        this.update_fut = Some(fut);
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
            self.map_unchecked_mut(|curried| match curried.update_fut.as_mut() {
                Some(fut) => fut.as_mut(),
                None => panic!()
            })
        };
        match fut.poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(()) => {
                todo!()
            }
        }
    }
}
