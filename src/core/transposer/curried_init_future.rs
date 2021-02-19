use super::{InitContext, context::LazyState, engine_time::EngineTime, transposer::Transposer, transposer_frame::TransposerFrame, update_result::UpdateResult};
use futures::channel::oneshot::{channel, Receiver, Sender};
use futures::Future;
use std::{
    marker::PhantomPinned,
    mem::MaybeUninit,
    pin::Pin,
    task::{Context, Poll},
};

pub(super) struct CurriedInitFuture<'a, T: Transposer + 'a> {
    // the curried future; placed first so it is dropped first.
    future: MaybeUninit<Box<dyn Future<Output = ()> + 'a>>,

    // cx is placed second because it references frame and is referenced by fut.
    context: MaybeUninit<InitContext<'a, T>>,

    // curried arguments to the internal future.
    frame: MaybeUninit<TransposerFrame<T>>,
    state: &'a mut LazyState<T::InputState>,

    // fut contains references to its curried arguments, so it can't be Unpin.
    _pin: PhantomPinned,
}

// lots of unsafe shenanegans goin on up in here
impl<'a, T: Transposer + 'a> CurriedInitFuture<'a, T> {
    pub fn new(transposer: &T, state: &'a mut LazyState<T::InputState>) -> Self {
        let mut frame = TransposerFrame::new((*transposer).clone());
        frame.internal.set_time(EngineTime::new_init());
        Self {
            future: MaybeUninit::uninit(),
            context: MaybeUninit::uninit(),
            frame: MaybeUninit::new(frame),
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

        // same with this
        let state_ref = unsafe {
            let ptr: *mut _ = this.state;
            ptr.as_mut().unwrap()
        };

        // create and initialize context
        let cx: InitContext<'a, T>;
        cx = InitContext::new(state_ref);
        this.context = MaybeUninit::new(cx);

        // take ref from newly pinned ref
        let cx_ref = unsafe {
            let ptr: *mut _ = this.context.as_mut_ptr();
            ptr.as_mut().unwrap()
        };

        // initialize update_fut
        let fut = frame_ref.transposer.init(cx_ref);
        let fut = Box::new(fut);
        this.future = MaybeUninit::new(fut);
    }
}

impl<'a, T: Transposer + 'a> Future for CurriedInitFuture<'a, T> {
    type Output = UpdateResult<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        let fut: Pin<&mut _> =
            unsafe { Pin::new_unchecked(this.future.as_mut_ptr().as_mut().unwrap().as_mut()) };
        match fut.poll(cx) {
            Poll::Ready(()) => {
                // destroy our future, polling after ready is not allowed anyway.
                this.future = MaybeUninit::uninit();

                let cx = std::mem::replace(&mut this.context, MaybeUninit::uninit());
                let cx = unsafe { cx.assume_init() };

                let frame = std::mem::replace(&mut this.frame, MaybeUninit::uninit());
                let frame = unsafe { frame.assume_init() };

                Poll::Ready(UpdateResult::from_init_context(frame, cx))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
