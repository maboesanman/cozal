use super::{UpdateContext, context::LazyState, engine_time::{EngineTime, EngineTimeSchedule}, transposer::Transposer, transposer_frame::TransposerFrame, update_result::UpdateResult};
use futures::channel::oneshot::{channel, Receiver, Sender};
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
    time: T::Time,
    payload: T::Scheduled,
    state: &'a mut LazyState<T::InputState>,

    // fut contains a reference to input, so we can't be Unpin
    _pin: PhantomPinned,
}

impl<'a, T: Transposer + 'a> CurriedScheduleFuture<'a, T> {
    pub fn new(
        prev_frame: &TransposerFrame<T>,
        state: &'a mut LazyState<T::InputState>,
    ) -> Self {
        let mut frame = (*prev_frame).clone();
        let (schedule_time, payload) = match frame.pop_schedule_event() {
            Some(x) => x,
            None => panic!("tried to start scheduled update with no events"),
        };
        let time = schedule_time.time;
        frame.internal.set_time(schedule_time.into());

        Self {
            update_fut: MaybeUninit::uninit(),
            update_cx: MaybeUninit::uninit(),
            frame: MaybeUninit::new(frame),
            time,
            payload,
            state,
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

        // and with this
        let payload_ref = unsafe {
            let ptr: *const _ = &this.payload;
            ptr.as_ref().unwrap()
        };

        // and with this
        let state_ref = unsafe {
            let ptr: *mut _ = this.state;
            ptr.as_mut().unwrap()
        };

        // prepare for updating our pinned struct

        // create and initialize context
        let cx: UpdateContext<'a, T>;
        cx = UpdateContext::new_scheduled(&mut frame_ref.internal, state_ref);
        this.update_cx = MaybeUninit::new(cx);

        // take ref from newly pinned ref
        let cx_ref = unsafe {
            let ptr: *mut _ = this.update_cx.as_mut_ptr();
            ptr.as_mut().unwrap()
        };

        let fut = frame_ref.transposer.handle_scheduled(this.time, payload_ref, cx_ref);
        let fut = Box::new(fut);
        this.update_fut = MaybeUninit::new(fut);
    }
}

impl<'a, T: Transposer + 'a> Future for CurriedScheduleFuture<'a, T> {
    type Output = UpdateResult<T>;

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

                Poll::Ready(UpdateResult::from_update_context(frame, update_cx))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
