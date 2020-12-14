use super::{UpdateContext, context::LazyState, internal_scheduled_event::InternalScheduledEvent, transposer::Transposer, transposer_frame::TransposerFrame, transposer_function_wrappers::WrappedUpdateResult};
use futures::{Future, future::Lazy};
use pin_project::pin_project;
use std::{marker::PhantomPinned, mem::MaybeUninit, pin::Pin, sync::Arc, task::{Context, Poll}};
use futures::channel::oneshot::{channel, Sender, Receiver};

#[pin_project]
pub(super) struct CurriedScheduleFuture<'a, T: Transposer + 'a> {
    // the curried future; placed first so it is dropped first.
    #[pin]
    update_fut: MaybeUninit<Box<dyn Future<Output = ()> + 'a>>,

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
    ) -> (Pin<Box<Self>>, Option<Sender<T::InputState>>) {
        let (state, sender) = match state {
            Some(s) => {
                (LazyState::Ready(s), None)
            }
            None => {
                let (send, recv) = channel();
                (LazyState::Pending(recv), Some(send))
            }
        };

        let mut pinned = Box::pin(Self {
            update_fut: MaybeUninit::uninit(),
            update_cx: MaybeUninit::uninit(),
            frame,
            event_arc: event_arc.clone(),
            state,
            _pin: PhantomPinned,
        });

        // this is safe because we are adjusting the lifetime
        // to be the lifetime of the pinned struct
        let frame_ref = unsafe {
            let ptr: *mut _ = &mut pinned.frame;
            ptr.as_mut().unwrap()
        };

        // and with this
        let event_ref = unsafe {
            let ptr: *const _ = &pinned.event_arc;
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
        cx = UpdateContext::new_scheduled(event_arc.time, &mut frame_ref.expire_handle_factory, state_ref);
        let mut_ref: Pin<&mut Self> = Pin::as_mut(&mut pinned);
        unsafe { Pin::get_unchecked_mut(mut_ref).update_cx = MaybeUninit::new(cx); }

        // take ref from newly pinned ref
        let cx_ref = unsafe {
            let ptr: *mut _ = pinned.update_cx.as_mut_ptr();
            ptr.as_mut().unwrap()
        };

        let fut = frame_ref.transposer.handle_scheduled(event_arc.time, &event_ref.payload, cx_ref);
        let fut = Box::new(fut);
        let mut_ref: Pin<&mut Self> = Pin::as_mut(&mut pinned);
        unsafe { Pin::get_unchecked_mut(mut_ref).update_fut = MaybeUninit::new(fut); }

        (pinned, sender)
    }

    pub fn recover_pinned(self: Pin<Box<Self>>) -> (Arc<InternalScheduledEvent<T>>, Option<T::InputState>) {
        let owned = unsafe { Pin::into_inner_unchecked(self) };
        (owned.event_arc, owned.state.destroy())
    }

    pub fn time(&self) -> T::Time {
        self.event_arc.time
    }
}

impl<'a, T: Transposer + 'a> Future for CurriedScheduleFuture<'a, T> {
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

