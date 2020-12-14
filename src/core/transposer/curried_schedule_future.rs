use super::{internal_scheduled_event::InternalScheduledEvent, transposer::Transposer, transposer_frame::TransposerFrame, transposer_function_wrappers::WrappedUpdateResult, transposer_function_wrappers::{handle_input, handle_scheduled}};
use futures::Future;
use pin_project::pin_project;
use std::{marker::PhantomPinned, pin::Pin, sync::Arc, task::{Context, Poll}};
use futures::channel::oneshot::{channel, Sender, Receiver};

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


#[pin_project]
pub(super) struct CurriedScheduleFuture<'a, T: Transposer + 'a> {
    // the curried future.
    #[pin]
    fut: Option<Box<dyn Future<Output = WrappedUpdateResult<T>> + 'a>>,

    // curried arguments to the internal future.
    frame: TransposerFrame<T>,
    event_arc: Arc<InternalScheduledEvent<T>>,
    // state: T::InputState,

    // fut contains a reference to input, so we can't be Unpin
    _pin: PhantomPinned,
}

impl<'a, T: Transposer + 'a> CurriedScheduleFuture<'a, T> {
    pub fn new(
        frame: TransposerFrame<T>,
        event_arc: Arc<InternalScheduledEvent<T>>,
        state: Option<T::InputState>,
    ) -> (Pin<Box<Self>>, Option<Sender<T::InputState>>) {
        let mut pinned = Box::pin(Self {
            frame,
            event_arc: event_arc.clone(),
            // state,
            fut: None,
            _pin: PhantomPinned,
        });

        // this is safe because we are adjusting the lifetime
        // to be the lifetime of the pinned struct
        let frame_ref = unsafe {
            let ptr: *mut _ = &mut pinned.frame;
            ptr.as_mut().unwrap()
        };
        let (sender, reciever) = channel();

        let sender = if let Some(state) = state {
            sender.send(state);
            None
        } else {
            Some(sender)
        };

        let fut = Box::new(handle_scheduled(frame_ref, event_arc));

        let mut_ref: Pin<&mut Self> = Pin::as_mut(&mut pinned);
        unsafe {
            // this is safe because this is a field of a pinned struct
            Pin::get_unchecked_mut(mut_ref).fut = Some(fut);
        }

        (pinned, sender)
    }

    pub fn recover_pinned(self: Pin<Box<Self>>) -> (Arc<InternalScheduledEvent<T>>, Option<T::InputState>) {
        // TODO this shouldn't be none
        (self.event_arc, None)
    }

    pub fn time(&self) -> T::Time {
        self.event_arc.time
    }
}

impl<'a, T: Transposer + 'a> Future for CurriedScheduleFuture<'a, T> {
    type Output = WrappedUpdateResult<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let fut = unsafe { self.map_unchecked_mut(|w| w.fut.as_deref_mut().unwrap()) };
        fut.poll(cx)
    }
}

