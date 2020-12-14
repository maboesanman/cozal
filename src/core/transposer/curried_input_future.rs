use super::{
    transposer::Transposer,
    transposer_frame::TransposerFrame, transposer_function_wrappers::handle_input,
    transposer_function_wrappers::WrappedUpdateResult,
};
use futures::Future;
use pin_project::pin_project;
use std::{marker::PhantomPinned, pin::Pin, task::{Context, Poll}};

#[pin_project]
pub(super) struct CurriedInputFuture<'a, T: Transposer + 'a> {
    // the curried future; placed first so it is dropped first.
    #[pin]
    fut: Option<Box<dyn Future<Output = WrappedUpdateResult<T>> + 'a>>,

    // curried arguments to the internal future.
    frame: TransposerFrame<T>,
    time: T::Time,
    inputs: Vec<T::Input>,
    state: T::InputState,

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
            frame,
            time,
            inputs,
            state,
            fut: None,
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
            let ptr: *const _ = &pinned.state;
            ptr.as_ref().unwrap()
        };

        let fut = Box::new(handle_input(frame_ref, time, input_ref, state_ref));

        let mut_ref: Pin<&mut Self> = Pin::as_mut(&mut pinned);
        unsafe {
            // this is safe because this is a field of a pinned struct
            Pin::get_unchecked_mut(mut_ref).fut = Some(fut);
        }

        pinned
    }

    // this does not recover frame because it may have been half-mutated
    pub fn recover_pinned(self: Pin<Box<Self>>) -> (T::Time, Vec<T::Input>, T::InputState) {
        let time = self.time;
        let inputs = unsafe { Pin::into_inner_unchecked(self).inputs };
        let state = self.state;
        (time, inputs, state)
    }

    pub fn time(&self) -> T::Time {
        self.time
    }
}

impl<'a, T: Transposer + 'a> Future for CurriedInputFuture<'a, T> {
    type Output = WrappedUpdateResult<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let fut = unsafe { self.map_unchecked_mut(|w| w.fut.as_deref_mut().unwrap()) };
        fut.poll(cx)
    }
}