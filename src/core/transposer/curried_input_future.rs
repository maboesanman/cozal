use super::{UpdateContext, context::LazyState, transposer::Transposer, transposer_frame::TransposerFrame, transposer_function_wrappers::WrappedUpdateResult};
use futures::Future;
use pin_project::pin_project;
use std::{marker::PhantomPinned, mem::MaybeUninit, pin::Pin, task::{Context, Poll}};

#[pin_project]
pub(super) struct CurriedInputFuture<'a, T: Transposer + 'a> {
    // the curried future; placed first so it is dropped first.
    #[pin]
    update_fut: MaybeUninit<Box<dyn Future<Output = ()> + 'a>>,

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
    ) -> Pin<Box<Self>> {
        let mut pinned = Box::pin(Self {
            update_fut: MaybeUninit::uninit(),
            update_cx: MaybeUninit::uninit(),
            frame,
            time,
            inputs,
            state: LazyState::Ready(state),
            _pin: PhantomPinned,
        });

        // this is safe because we are adjusting the lifetime
        // to be the lifetime of the pinned struct
        let frame_ref = unsafe {
            let ptr: *mut _ = &mut pinned.frame;
            ptr.as_mut().unwrap()
        };

        // same with this
        let input_ref = unsafe {
            let ptr: *const _ = &pinned.inputs;
            ptr.as_ref().unwrap()
        };

        // and with this
        let state_ref = unsafe {
            let ptr: *mut _ = &mut pinned.state;
            ptr.as_mut().unwrap()
        };

        // prepare for updating our pinned struct

        // create and initialize context
        let cx: UpdateContext<'a, T>;
        cx = UpdateContext::new_input(time, &mut frame_ref.expire_handle_factory, state_ref);
        let mut_ref: Pin<&mut Self> = Pin::as_mut(&mut pinned);
        unsafe { Pin::get_unchecked_mut(mut_ref).update_cx = MaybeUninit::new(cx); }

        // take ref from newly pinned ref
        let cx_ref = unsafe {
            let ptr: *mut _ = pinned.update_cx.as_mut_ptr();
            ptr.as_mut().unwrap()
        };

        // initialize update_fut
        let fut = frame_ref.transposer.handle_input(time, input_ref, cx_ref);
        let fut = Box::new(fut);
        let mut_ref: Pin<&mut Self> = Pin::as_mut(&mut pinned);
        unsafe { Pin::get_unchecked_mut(mut_ref).update_fut = MaybeUninit::new(fut); }

        pinned
    }



    // this does not recover frame because it may have been half-mutated
    pub fn recover_pinned(self: Pin<Box<Self>>) -> (T::Time, Vec<T::Input>, T::InputState) {
        let owned = unsafe { Pin::into_inner_unchecked(self) };
        let state = match owned.state.destroy() {
            Some(s) => s,
            None => unreachable!()
        };
        (owned.time, owned.inputs, state)
    }

    pub fn time(&self) -> T::Time {
        self.time
    }
}

impl<'a, T: Transposer + 'a> Future for CurriedInputFuture<'a, T> {
    type Output = WrappedUpdateResult<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let fut = unsafe { self.map_unchecked_mut(|w|
            w.update_fut
            .as_mut_ptr()
            .as_mut().unwrap()
            .as_mut()
        ) };
        match fut.poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(()) => {
                todo!()
            }
        }
    }
}