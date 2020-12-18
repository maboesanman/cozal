use super::{
    context::LazyState,
    internal_scheduled_event::{InternalScheduledEvent, Source},
    transposer::Transposer,
    transposer_frame::TransposerFrame,
    transposer_function_wrappers::WrappedUpdateResult,
    UpdateContext,
};
use futures::channel::oneshot::{channel, Sender};
use futures::Future;
use std::{
    marker::PhantomPinned,
    mem::MaybeUninit,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

pub(super) struct CurriedScheduleFuture<'a, T: Transposer + 'a> {
    // the curried future; placed first so it is dropped first.
    update_fut: MaybeUninit<Box<dyn Future<Output = ()> + 'a>>,

    // cx is placed second because it references frame and is referenced by fut.
    update_cx: MaybeUninit<UpdateContext<'a, T>>,

    // curried arguments to the internal future.
    frame: MaybeUninit<TransposerFrame<T>>,
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
    ) -> (Self, Option<Sender<T::InputState>>) {
        let (state, state_sender) = match state {
            Some(s) => (LazyState::Ready(s), None),
            None => {
                let (state_sender, state_reciever) = channel();
                (LazyState::Pending(state_reciever), Some(state_sender))
            }
        };

        let new_self = Self {
            update_fut: MaybeUninit::uninit(),
            update_cx: MaybeUninit::uninit(),
            frame: MaybeUninit::new(frame),
            event_arc,
            state,
            _pin: PhantomPinned,
        };

        (new_self, state_sender)
    }

    pub fn init(self: Pin<&mut Self>, notification_reciever: Option<Sender<()>>) {
        // this is safe because we are adjusting the lifetime
        // to be the lifetime of the pinned struct

        let this = unsafe { self.get_unchecked_mut() };

        let frame_ref = unsafe {
            let ptr: *mut _ = this.frame.as_mut_ptr();
            ptr.as_mut().unwrap()
        };

        // and with this
        let event_ref = unsafe {
            let ptr: *const _ = &this.event_arc;
            ptr.as_ref().unwrap()
        };

        // and with this
        let state_ref = unsafe {
            let ptr: *mut _ = &mut this.state;
            ptr.as_mut().unwrap()
        };

        // prepare for updating our pinned struct

        // create and initialize context
        let cx: UpdateContext<'a, T>;
        cx = UpdateContext::new_scheduled(
            this.event_arc.clone(),
            &mut frame_ref.schedule,
            &mut frame_ref.expire_handles,
            &mut frame_ref.expire_handle_factory,
            state_ref,
            notification_reciever,
        );
        this.update_cx = MaybeUninit::new(cx);

        // take ref from newly pinned ref
        let cx_ref = unsafe {
            let ptr: *mut _ = this.update_cx.as_mut_ptr();
            ptr.as_mut().unwrap()
        };

        let fut =
            frame_ref
                .transposer
                .handle_scheduled(this.event_arc.time, &event_ref.payload, cx_ref);
        let fut = Box::new(fut);
        this.update_fut = MaybeUninit::new(fut);
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
