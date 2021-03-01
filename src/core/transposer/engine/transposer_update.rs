use futures::Future;
use std::{
    marker::PhantomPinned,
    mem::MaybeUninit,
    pin::Pin,
    task::{Context, Poll},
};

use crate::core::Transposer;

use super::{engine_context::{EngineContext}, lazy_state::LazyState, transposer_frame::TransposerFrame, update_result::UpdateResult};

/// future to initialize a TransposerFrame
///
/// the initialization happens AS A SIDE EFFECT OF THIS.
pub(super) struct TransposerUpdate<'f, T: Transposer>
where T::Scheduled: Clone {
    // the curried future; placed first so it is dropped first.
    future: MaybeUninit<Box<dyn Future<Output = ()> + 'f>>,

    // cx is placed second because it references frame and is referenced by fut.
    context: MaybeUninit<EngineContext<'f, T>>,

    // future contains a reference to context.
    _pin: PhantomPinned,
}

// lots of unsafe shenanegans goin on up in here
impl<'f, T: Transposer + Clone> TransposerUpdate<'f, T> 
where T::Scheduled: Clone {
    pub fn new() -> Self {
        Self {
            future: MaybeUninit::uninit(),
            context: MaybeUninit::uninit(),
            _pin: PhantomPinned,
        }
    }

    fn setup_helper<'s, 'a>(
        self: Pin<&'s mut Self>,
        frame: &'f mut TransposerFrame<'f, T>,
        state: &'f mut LazyState<T::InputState>,
    ) -> (
        &'s mut MaybeUninit<Box<dyn Future<Output = ()> + 'f>>,
        &'a mut T,
        &'a mut EngineContext<'f, T>
    ) {
        // this is safe because we are adjusting the lifetime
        // to be the lifetime of the pinned struct
        let this: &mut Self = unsafe { self.get_unchecked_mut() };

        // create and initialize context
        let cx: EngineContext<'f, T>;
        cx = EngineContext::new(&mut frame.internal, state);
        this.context = MaybeUninit::new(cx);

        // take ref from newly pinned ref
        let cx_ref = unsafe {
            let ptr: *mut _ = this.context.as_mut_ptr();
            ptr.as_mut().unwrap()
        };

        let transposer_ref = &mut frame.transposer;
        (&mut this.future, transposer_ref, cx_ref)
    }

    pub fn start_init(
        mut self: Pin<&mut Self>, 
        frame: &'f mut TransposerFrame<'f, T>,
        state: &'f mut LazyState<T::InputState>,
    )
    {
        let (future_ref, transposer_ref, cx_ref) = self.as_mut().setup_helper(frame, state);
        // initialize update_fut
        let fut = transposer_ref.init(cx_ref);
        let fut = Box::new(fut);
        *future_ref = MaybeUninit::new(fut);
    }

    pub fn start_input(
        mut self: Pin<&mut Self>, 
        frame: &'f mut TransposerFrame<'f, T>,
        state: &'f mut LazyState<T::InputState>,
        time: T::Time,
        inputs: &'f [T::Input]
    )
    {
        let (future_ref, transposer_ref, cx_ref) = self.as_mut().setup_helper(frame, state);
        // initialize update_fut
        let fut = transposer_ref.handle_input(time, inputs, cx_ref);
        let fut = Box::new(fut);
        *future_ref = MaybeUninit::new(fut);
    }

    pub fn start_schedule(
        mut self: Pin<&mut Self>, 
        frame: &'f mut TransposerFrame<'f, T>,
        state: &'f mut LazyState<T::InputState>
    )
    {
        let (schedule_time, payload) = frame.pop_schedule_event().unwrap();
        let (future_ref, transposer_ref, cx_ref) = self.as_mut().setup_helper(frame, state);
        // initialize update_fut
        let fut = transposer_ref.handle_scheduled(schedule_time.time, payload, cx_ref);
        let fut = Box::new(fut);
        *future_ref = MaybeUninit::new(fut);
    }
}

impl<'a, T: Transposer + Clone> Future for TransposerUpdate<'a, T> 
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
