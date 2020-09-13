use crate::core::schedule_stream::SchedulePoll;

use super::{
    internal_scheduled_event::InternalScheduledEvent, transposer::Transposer,
    transposer_frame::TransposerFrame, transposer_function_wrappers::handle_input,
    transposer_function_wrappers::handle_scheduled,
    transposer_function_wrappers::WrappedUpdateResult,
};
use futures::Future;
use pin_project::pin_project;
use std::{
    marker::PhantomPinned,
    pin::Pin,
    sync::Arc,
    sync::RwLock,
    task::{Context, Poll},
};

pub(super) enum TransposerUpdate<'a, T: Transposer> {
    Input(Pin<Box<CurriedInputFuture<'a, T>>>),
    Schedule(Pin<Box<CurriedScheduleFuture<'a, T>>>),
    WaitingInput((T::Time, Vec<T::Input>, WrappedUpdateResult<T>)),
    WaitingScheduled((T::Time, WrappedUpdateResult<T>)),
    None,
}

impl<'a, T: Transposer> Default for TransposerUpdate<'a, T> {
    fn default() -> Self {
        Self::None
    }
}

impl<'a, T: Transposer> TransposerUpdate<'a, T> {
    pub fn new_input(
        frame_arc: Arc<RwLock<TransposerFrame<T>>>,
        time: T::Time,
        inputs: Vec<T::Input>,
    ) -> Self {
        TransposerUpdate::Input(CurriedInputFuture::new(frame_arc, time, inputs))
    }

    pub fn new_schedule(
        frame_arc: Arc<RwLock<TransposerFrame<T>>>,
        event_arc: Arc<InternalScheduledEvent<T>>,
    ) -> Self {
        TransposerUpdate::Schedule(Box::pin(CurriedScheduleFuture::new(frame_arc, event_arc)))
    }

    pub fn poll(
        &mut self,
        poll_time: T::Time,
        cx: &mut Context<'_>,
    ) -> SchedulePoll<T::Time, (WrappedUpdateResult<T>, T::Time, Vec<T::Input>)> {
        loop {
            match self {
                Self::Input(input_fut) => match input_fut.as_mut().poll(cx) {
                    Poll::Ready(result) => {
                        if let Self::Input(fut) = std::mem::take(self) {
                            let (time, inputs) = fut.recover_pinned();
                            *self = Self::WaitingInput((time, inputs, result));
                        }
                    }
                    Poll::Pending => break SchedulePoll::Pending,
                },
                Self::Schedule(schedule_fut) => match schedule_fut.as_mut().poll(cx) {
                    Poll::Ready(result) => {
                        if let Self::Schedule(fut) = std::mem::take(self) {
                            let time = fut.time();
                            *self = Self::WaitingScheduled((time, result));
                        }
                    }
                    Poll::Pending => break SchedulePoll::Pending,
                },
                Self::WaitingInput((time, _, _)) => {
                    if *time > poll_time {
                        break SchedulePoll::Scheduled(*time);
                    }

                    if let Self::WaitingInput((time, inputs, result)) = std::mem::take(self) {
                        break SchedulePoll::Ready((result, time, inputs));
                    }

                    unreachable!()
                }
                Self::WaitingScheduled((time, _)) => {
                    if *time > poll_time {
                        break SchedulePoll::Scheduled(*time);
                    }

                    if let Self::WaitingScheduled((time, result)) = std::mem::take(self) {
                        break SchedulePoll::Ready((result, time, Vec::new()));
                    }

                    unreachable!()
                }
                Self::None => break SchedulePoll::Pending,
            }
        }
    }

    pub fn is_some(&self) -> bool {
        !matches!(self, Self::None)
    }

    #[allow(dead_code)]
    pub fn unstage(&mut self) -> Option<(T::Time, Vec<T::Input>)> {
        match std::mem::take(self) {
            Self::Input(fut) => Some(fut.recover_pinned()),
            Self::Schedule(fut) => Some((fut.time(), Vec::new())),
            Self::WaitingInput((time, inputs, _)) => Some((time, inputs)),
            Self::WaitingScheduled((time, _)) => Some((time, Vec::new())),
            Self::None => None,
        }
    }
}

pub(super) struct CurriedInputFuture<'a, T: Transposer + 'a> {
    // curried arguments to the internal future.
    time: T::Time,
    inputs: Vec<T::Input>,

    // the curried future.
    fut: Option<Box<dyn Future<Output = WrappedUpdateResult<T>> + 'a>>,

    // fut contains a reference to input, so we can't be Unpin
    _pin: PhantomPinned,
}

impl<'a, T: Transposer + 'a> CurriedInputFuture<'a, T> {
    pub fn new(
        frame_arc: Arc<RwLock<TransposerFrame<T>>>,
        time: T::Time,
        inputs: Vec<T::Input>,
    ) -> Pin<Box<Self>> {
        let mut pinned = Box::pin(Self {
            time,
            inputs,
            fut: None,
            _pin: PhantomPinned,
        });

        let input_ref = unsafe {
            // this is safe because we are adjusting the lifetime
            // to be the lifetime of the pinned struct
            let ptr: *const _ = &pinned.inputs;
            ptr.as_ref().unwrap()
        };

        let fut = Box::new(handle_input(frame_arc, time, input_ref));

        let mut_ref: Pin<&mut Self> = Pin::as_mut(&mut pinned);
        unsafe {
            // this is safe because this is a field of a pinned struct
            Pin::get_unchecked_mut(mut_ref).fut = Some(fut);
        }

        pinned
    }

    pub fn recover_pinned(self: Pin<Box<Self>>) -> (T::Time, Vec<T::Input>) {
        let time = self.time;
        let inputs = unsafe { Pin::into_inner_unchecked(self).inputs };
        (time, inputs)
    }

    #[allow(dead_code)]
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
    // curried arguments to the internal future.
    time: T::Time,

    // the curried future.
    #[pin]
    fut: Pin<Box<dyn Future<Output = WrappedUpdateResult<T>> + 'a>>,
}

impl<'a, T: Transposer + 'a> CurriedScheduleFuture<'a, T> {
    pub fn new(
        frame_arc: Arc<RwLock<TransposerFrame<T>>>,
        event_arc: Arc<InternalScheduledEvent<T>>,
    ) -> Self {
        let time = event_arc.time;
        let fut = Box::pin(handle_scheduled(frame_arc, event_arc));

        Self { time, fut }
    }

    pub fn time(&self) -> T::Time {
        self.time
    }
}

impl<'a, T: Transposer + 'a> Future for CurriedScheduleFuture<'a, T> {
    type Output = WrappedUpdateResult<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        self.project().fut.poll(cx)
    }
}
