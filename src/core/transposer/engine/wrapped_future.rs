use futures::Future;
use std::{
    marker::PhantomPinned,
    mem::MaybeUninit,
    pin::Pin,
    task::{Context, Poll},
};

use crate::core::Transposer;

use super::{engine_context::{EngineContext, LazyState}, engine_time::EngineTime, transposer_frame::TransposerFrame, update_result::UpdateResult};

/// future to initialize a TransposerFrame
///
/// the initialization happens AS A SIDE EFFECT OF THIS.
pub(super) struct WrappedFuture<'a, T: Transposer> 
where T::Scheduled: Clone {
    // the curried future; placed first so it is dropped first.
    future: MaybeUninit<Box<dyn Future<Output = ()> + 'a>>,

    // cx is placed second because it references frame and is referenced by fut.
    context: MaybeUninit<EngineContext<'a, T>>,

    // curried arguments to the internal future.
    frame: &'a mut TransposerFrame<T>,

    // fut contains references to its curried arguments, so it can't be Unpin.
    _pin: PhantomPinned,
}

// lots of unsafe shenanegans goin on up in here
impl<'a, T: Transposer + Clone> WrappedFuture<'a, T> 
where T::Scheduled: Clone {
    pub fn new(
        frame: &'a mut TransposerFrame<T>,
    ) -> Self {
        frame.internal.set_time(EngineTime::new_init());
        Self {
            future: MaybeUninit::uninit(),
            context: MaybeUninit::uninit(),
            frame,
            _pin: PhantomPinned,
        }
    }

    fn setup_helper<'f, 'b>(
        self: Pin<&'f mut Self>, 
        state: &'a mut LazyState<T::InputState>,
    ) -> (
        &'f mut MaybeUninit<Box<dyn Future<Output = ()> + 'a>>,
        &'b mut T,
        &'b mut EngineContext<'a, T>
    ) {
        // this is safe because we are adjusting the lifetime
        // to be the lifetime of the pinned struct
        let this: &mut Self = unsafe { self.get_unchecked_mut() };

        let frame_ref = unsafe {
            let ptr: *mut _ = this.frame;
            ptr.as_mut().unwrap()
        };

        // create and initialize context
        let cx: EngineContext<'a, T>;
        cx = EngineContext::new(&mut frame_ref.internal, state);
        this.context = MaybeUninit::new(cx);

        // take ref from newly pinned ref
        let cx_ref = unsafe {
            let ptr: *mut _ = this.context.as_mut_ptr();
            ptr.as_mut().unwrap()
        };

        let transposer_ref = &mut frame_ref.transposer;
        (&mut this.future, transposer_ref, cx_ref)
    }

    pub fn start_init(
        mut self: Pin<&mut Self>, 
        state: &'a mut LazyState<T::InputState>,
    )
    {
        let (future_ref, transposer_ref, cx_ref) = self.as_mut().setup_helper(state);
        // initialize update_fut
        let fut = transposer_ref.init(cx_ref);
        let fut = Box::new(fut);
        *future_ref = MaybeUninit::new(fut);
    }

    pub fn start_input(
        mut self: Pin<&mut Self>, 
        state: &'a mut LazyState<T::InputState>,
        time: T::Time,
        inputs: &'a [T::Input]
    )
    {
        let (future_ref, transposer_ref, cx_ref) = self.as_mut().setup_helper(state);
        // initialize update_fut
        let fut = transposer_ref.handle_input(time, inputs, cx_ref);
        let fut = Box::new(fut);
        *future_ref = MaybeUninit::new(fut);
    }

    pub fn start_schedule(
        mut self: Pin<&mut Self>, 
        state: &'a mut LazyState<T::InputState>,
        time: T::Time,
        payload: T::Scheduled,
    )
    {
        let (future_ref, transposer_ref, cx_ref) = self.as_mut().setup_helper(state);
        // initialize update_fut
        let fut = transposer_ref.handle_scheduled(time, payload, cx_ref);
        let fut = Box::new(fut);
        *future_ref = MaybeUninit::new(fut);
    }
}

impl<'a, T: Transposer + Clone> Future for WrappedFuture<'a, T> 
where T::Scheduled: Clone {
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

                Poll::Ready(cx.into())
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
