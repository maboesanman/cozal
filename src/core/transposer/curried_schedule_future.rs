use super::{
    context::LazyState, internal_scheduled_event::InternalScheduledEvent, transposer::Transposer,
    transposer_frame::TransposerFrame, transposer_function_wrappers::WrappedUpdateResult,
    UpdateContext,
};
use futures::channel::oneshot::{channel, Receiver, Sender};
use futures::{future::Lazy, Future};
use pin_project::pin_project;
use std::{
    marker::PhantomPinned,
    mem::MaybeUninit,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

pub(super) struct CurriedScheduleFuture<'a, T: Transposer + 'a> {
    // the curried future; placed first so it is dropped first.
    update_fut: Option<Box<dyn Future<Output = ()> + 'a>>,

    // cx is placed second because it references frame and is referenced by fut.
    update_cx: MaybeUninit<UpdateContext<'a, T>>,

    // curried arguments to the internal future.
    frame: TransposerFrame<T>,
    event_arc: Arc<InternalScheduledEvent<T>>,
    state: LazyState<T::InputState>,

    // fut contains a reference to input, so we can't be Unpin
    _pin: PhantomPinned,
}

impl<'a, T: Transposer + 'a> CurriedScheduleFuture<'a, T> {
    pub fn new(
        frame: TransposerFrame<T>,
        event_arc: Arc<InternalScheduledEvent<T>>,
        state: Option<T::InputState>,
    ) -> (
        Self,
        Option<Sender<T::InputState>>,
    ) {
        let (state, state_sender) = match state {
            Some(s) => (LazyState::Ready(s), None),
            None => {
                let (state_sender, state_reciever) = channel();
                (
                    LazyState::Pending(state_reciever),
                    Some(state_sender),
                )
            }
        };

        let new_self = Self {
            update_fut: None,
            update_cx: MaybeUninit::uninit(),
            frame,
            event_arc: event_arc.clone(),
            state,
            _pin: PhantomPinned,
        };

        (new_self, state_sender)
    }

    pub fn init(self: Pin<&mut Self>, notification_reciever: Option<Sender<()>>) {

        // this is safe because we are adjusting the lifetime
        // to be the lifetime of the pinned struct
        let frame_ref = unsafe {
            let ptr: *mut _ = &mut self.frame;
            ptr.as_mut().unwrap()
        };

        // and with this
        let event_ref = unsafe {
            let ptr: *const _ = &self.event_arc;
            ptr.as_ref().unwrap()
        };

        // and with this
        let state_ref = unsafe {
            let ptr: *mut _ = &mut self.state;
            ptr.as_mut().unwrap()
        };

        // prepare for updating our pinned struct

        // create and initialize context
        let cx: UpdateContext<'a, T>;
        cx = UpdateContext::new_scheduled(
            self.event_arc.time,
            &mut frame_ref.expire_handle_factory,
            state_ref,
            notification_reciever,
        );
        let mut_ref: Pin<&mut Self> = Pin::as_mut(&mut self);
        unsafe {
            Pin::get_unchecked_mut(mut_ref).update_cx = MaybeUninit::new(cx);
        }

        // take ref from newly pinned ref
        let cx_ref = unsafe {
            let ptr: *mut _ = self.update_cx.as_mut_ptr();
            ptr.as_mut().unwrap()
        };

        let fut = frame_ref
            .transposer
            .handle_scheduled(self.event_arc.time, &event_ref.payload, cx_ref);
        let fut = Box::new(fut);
        let mut_ref: Pin<&mut Self> = Pin::as_mut(&mut self);
        unsafe {
            Pin::get_unchecked_mut(mut_ref).update_fut = Some(fut);
        }
    }

    pub fn recover_pinned(self: Pin<&mut Self>) -> (Arc<InternalScheduledEvent<T>>, Option<T::InputState>) {
        let owned = unsafe { Pin::into_inner_unchecked(self)};
        owned.recover()
    }

    pub fn recover(self) -> (Arc<InternalScheduledEvent<T>>, Option<T::InputState>) {
        (self.event_arc, self.state.destroy())
    }

    pub fn time(&self) -> T::Time {
        self.event_arc.time
    }
}

impl<'a, T: Transposer + 'a> Future for CurriedScheduleFuture<'a, T> {
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
