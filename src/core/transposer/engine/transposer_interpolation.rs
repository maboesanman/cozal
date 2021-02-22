use futures::Future;
use std::{
    marker::PhantomPinned,
    mem::MaybeUninit,
    pin::Pin,
    task::{Context, Poll},
};

use crate::core::Transposer;

use super::{engine_context::{EngineContext, EngineInterpolationContext, LazyState}, engine_time::EngineTime, transposer_frame::TransposerFrame, update_result::UpdateResult};

/// future to initialize a TransposerFrame
///
/// the initialization happens AS A SIDE EFFECT OF THIS.
pub(super) struct TransposerInterpolation<'f, T: Transposer> 
where T::Scheduled: Clone {
    // the curried future; placed first so it is dropped first.
    future: MaybeUninit<Box<dyn Future<Output = T::OutputState> + 'f>>,

    // cx is placed second because it references frame and is referenced by fut.
    context: MaybeUninit<EngineInterpolationContext<'f, T>>,

    // future contains a reference to context.
    _pin: PhantomPinned,
}

// lots of unsafe shenanegans goin on up in here
impl<'f, T: Transposer + Clone> TransposerInterpolation<'f, T> 
where T::Scheduled: Clone {
    pub fn new() -> Self {
        Self {
            future: MaybeUninit::uninit(),
            context: MaybeUninit::uninit(),
            _pin: PhantomPinned,
        }
    }

    pub fn start_interpolation(
        mut self: Pin<&mut Self>,
        frame: &'f TransposerFrame<T>,
        state: &'f mut LazyState<T::InputState>,
        poll_time: T::Time,
    )
    {
        // this is safe because we are adjusting the lifetime
        // to be the lifetime of the pinned struct
        let this: &mut Self = unsafe { self.get_unchecked_mut() };

        // create and initialize context
        let cx: EngineInterpolationContext<'f, T>;
        cx = EngineInterpolationContext::new(state);
        this.context = MaybeUninit::new(cx);

        // take ref from newly pinned ref
        let cx_ref = unsafe {
            let ptr: *mut _ = this.context.as_mut_ptr();
            ptr.as_mut().unwrap()
        };

        let base_time = frame.internal.get_time().raw_time();

        // initialize update_fut
        let fut = frame.transposer.interpolate(base_time, poll_time, cx_ref);
        let fut = Box::new(fut);
        this.future = MaybeUninit::new(fut);
    }
}

impl<'a, T: Transposer + Clone> Future for TransposerInterpolation<'a, T> 
where T::Scheduled: Clone {
    type Output = T::OutputState;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        let fut: Pin<&mut _> =
            unsafe { Pin::new_unchecked(this.future.as_mut_ptr().as_mut().unwrap().as_mut()) };
        fut.poll(cx)
    }
}
