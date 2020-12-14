use super::{curried_input_future::CurriedInputFuture, curried_schedule_future::CurriedScheduleFuture, internal_scheduled_event::InternalScheduledEvent, transposer::Transposer, transposer_frame::TransposerFrame, transposer_function_wrappers::WrappedUpdateResult, transposer_function_wrappers::handle_input, transposer_function_wrappers::handle_scheduled};
use futures::{Future, channel::oneshot::Sender};
use pin_project::pin_project;
use std::{marker::PhantomPinned, pin::Pin, sync::Arc, sync::RwLock, task::{Context, Poll}};

pub(super) enum TransposerUpdate<'a, T: Transposer> {
    // Input event processing has begun; future has not returned yet.
    Input(Pin<Box<CurriedInputFuture<'a, T>>>),

    // Schedule event processing has begun; future has not returned yet.
    Schedule(Pin<Box<CurriedScheduleFuture<'a, T>>>),

    // Input event processing has finished processing; resources are released.
    ReadyInput((T::Time, Vec<T::Input>, T::InputState, WrappedUpdateResult<T>)),

    // Schedule event processing has finished processing; resources are released.
    ReadyScheduled((T::Time, Option<T::InputState>, WrappedUpdateResult<T>)),

    // there is no update.
    None,
}

impl<'a, T: Transposer> Default for TransposerUpdate<'a, T> {
    fn default() -> Self {
        Self::None
    }
}

impl<'a, T: Transposer> TransposerUpdate<'a, T> {
    pub fn new_input(
        frame: TransposerFrame<T>,
        time: T::Time,
        inputs: Vec<T::Input>,
        state: T::InputState,
    ) -> Self {
        TransposerUpdate::Input(CurriedInputFuture::new(frame, time, inputs, state))
    }

    pub fn new_schedule(
        frame: TransposerFrame<T>,
        event_arc: Arc<InternalScheduledEvent<T>>,
        state: Option<T::InputState>,
    ) -> (Self, Option<Sender<T::InputState>>) {
        let (fut, sender) = CurriedScheduleFuture::new(frame, event_arc, state);
        (TransposerUpdate::Schedule(fut), sender)
    }

    // fn get_time(&self) -> Option<T::Time> {
    //     match self {
    //         TransposerUpdate::Input(i) => Some(i.time()),
    //         TransposerUpdate::Schedule(u) => Some(u.time()),
    //         TransposerUpdate::ReadyInput((t, ..)) => Some(*t),
    //         TransposerUpdate::ReadyScheduled((t, ..)) => Some(*t),
    //         TransposerUpdate::None => None 
    //     }
    // }

    pub fn poll(
        &mut self,
        poll_time: T::Time,
        input_state: Option<&T::InputState>,
        cx: &mut Context<'_>,
    ) -> TransposerUpdatePoll<T> {
        // loop {
        //     match self {
        //         Self::Input(input_fut) => match input_fut.as_mut().poll(cx) {
        //             Poll::Ready(result) => {
        //                 if let Self::Input(fut) = std::mem::take(self) {
        //                     let (time, inputs, in_state) = fut.recover_pinned();
        //                     *self = Self::ReadyInput((time, inputs, in_state, result));
        //                 }
        //             }
        //             Poll::Pending => break TransposerUpdatePoll::Pending,
        //         },
        //         Self::Schedule(schedule_fut) => match schedule_fut.as_mut().poll(cx) {
        //             // this needs to somehow figure out that the state has been requested
        //             Poll::Ready(result) => {
        //                 if let Self::Schedule(fut) = std::mem::take(self) {
        //                     let (time, in_state) = fut.recover_pinned();
        //                     *self = Self::ReadyScheduled((time, in_state, result));
        //                 }
        //             }
        //             Poll::Pending => break TransposerUpdatePoll::Pending,
        //         },
        //         Self::ReadyInput((time, ..)) => {
        //             if *time > poll_time {
        //                 break TransposerUpdatePoll::Scheduled(*time);
        //             }

        //             // this replaces self with the none variant, decomposes the current ready input, and returns it in the poll
        //             if let Self::ReadyInput((time, inputs, in_state, result)) = std::mem::take(self) {
        //                 break TransposerUpdatePoll::Ready((result, time, inputs));
        //             }

        //             unreachable!()
        //         }
        //         Self::ReadyScheduled((time, ..)) => {
        //             if *time > poll_time {
        //                 break TransposerUpdatePoll::Scheduled(*time);
        //             }

        //             // this replaces self with the none variant, decomposes the current ready input, and returns it in the poll
        //             if let Self::ReadyScheduled((time, in_state, result)) = std::mem::take(self) {
        //                 break TransposerUpdatePoll::Ready((result, time, Vec::new()));
        //             }

        //             unreachable!()
        //         }
        //         Self::None => break TransposerUpdatePoll::Pending,
        //     }
        // }
        todo!()
    }

    pub fn is_some(&self) -> bool {
        !matches!(self, Self::None)
    }

    #[allow(dead_code)]
    pub fn unstage(&mut self) -> Option<(T::Time, Vec<T::Input>, Option<T::InputState>)> {
        // match std::mem::take(self) {
        //     Self::Input(fut) => {
        //         let (time, inputs, in_state) = fut.recover_pinned();
        //         Some((time, inputs, Some(in_state)))
        //     },
        //     Self::Schedule(fut) => {
        //         let (time, in_state) = fut.recover_pinned();
        //         Some((time, Vec::new(), in_state))
        //     }
        //     Self::ReadyInput((time, inputs, in_state, _update_result)) => {
        //         Some((time, inputs, Some(in_state)))
        //     },
        //     Self::ReadyScheduled((time, in_state, _update_result)) => {
        //         Some((time, Vec::new(), in_state))
        //     },
        //     Self::None => None,
        // }
        todo!()
    }
}

pub(super) enum TransposerUpdatePoll<T: Transposer> {
    Ready((WrappedUpdateResult<T>, T::Time, Vec<T::Input>)),
    Scheduled(T::Time),
    NeedsState,
    Pending,
}

