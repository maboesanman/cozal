use super::{
    context::LazyState, internal_scheduled_event::Source, transposer::Transposer,
    transposer_frame::TransposerFrame, wrapped_update_result::WrappedUpdateResult, UpdateContext,
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
    update_fut: MaybeUninit<Box<dyn Future<Output = ()> + 'a>>,

    // cx is placed second because it references frame and is referenced by fut.
    update_cx: MaybeUninit<UpdateContext<'a, T>>,

    // curried arguments to the internal future.
    frame: MaybeUninit<TransposerFrame<T>>,
    time: T::Time,
    inputs: Vec<T::Input>,
    state: LazyState<T::InputState>,

    // fut contains references to its curried arguments, so it can't be Unpin.
    _pin: PhantomPinned,
}

// lots of unsafe shenanegans goin on up in here
impl<'a, T: Transposer + 'a> CurriedInputFuture<'a, T> {
    pub fn new(
        mut frame: TransposerFrame<T>,
        time: T::Time,
        inputs: Vec<T::Input>,
        state: T::InputState,
    ) -> Self {
        frame.internal.set_source(Source::Input(time));
        Self {
            update_fut: MaybeUninit::uninit(),
            update_cx: MaybeUninit::uninit(),
            frame: MaybeUninit::new(frame),
            time,
            inputs,
            state: LazyState::Ready(state),
            _pin: PhantomPinned,
        }
    }

    pub fn init(self: Pin<&mut Self>) {
        // this is safe because we are adjusting the lifetime
        // to be the lifetime of the pinned struct
        let this = unsafe { self.get_unchecked_mut() };

        let frame_ref = unsafe {
            let ptr: *mut _ = this.frame.as_mut_ptr();
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
        cx = UpdateContext::new_input(&mut frame_ref.internal, state_ref);
        this.update_cx = MaybeUninit::new(cx);

        // take ref from newly pinned ref
        let cx_ref = unsafe {
            let ptr: *mut _ = this.update_cx.as_mut_ptr();
            ptr.as_mut().unwrap()
        };

        // initialize update_fut
        let fut = frame_ref
            .transposer
            .handle_input(this.time, input_ref, cx_ref);
        let fut = Box::new(fut);
        this.update_fut = MaybeUninit::new(fut);
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
        let this = unsafe { self.get_unchecked_mut() };
        let update_fut: Pin<&mut _> =
            unsafe { Pin::new_unchecked(this.update_fut.as_mut_ptr().as_mut().unwrap().as_mut()) };
        match update_fut.poll(cx) {
            Poll::Ready(()) => {
                // rip apart our future, polling after ready is not allowed anyway.
                let update_fut = std::mem::replace(&mut this.update_fut, MaybeUninit::uninit());
                std::mem::drop(update_fut);

                let update_cx = std::mem::replace(&mut this.update_cx, MaybeUninit::uninit());
                let update_cx = unsafe { update_cx.assume_init() };

                let frame = std::mem::replace(&mut this.frame, MaybeUninit::uninit());
                let frame = unsafe { frame.assume_init() };

                Poll::Ready(WrappedUpdateResult::new(frame, update_cx))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
