use super::{
    curried_input_future::CurriedInputFuture, curried_schedule_future::CurriedScheduleFuture,
    internal_scheduled_event::InternalScheduledEvent, transposer::Transposer,
    transposer_frame::TransposerFrame, transposer_function_wrappers::WrappedUpdateResult,
};
use futures::{
    channel::oneshot::{Receiver, Sender},
    Future,
};
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

pub(super) enum TransposerUpdate<'a, T: Transposer> {
    // Input event processing has begun; future has not returned yet.
    Input(Pin<Box<CurriedInputFuture<'a, T>>>),

    // Schedule event processing has begun; future has not returned yet.
    Schedule(
        (
            Pin<Box<CurriedScheduleFuture<'a, T>>>,
            Option<(Sender<T::InputState>, Receiver<()>)>,
        ),
    ),

    // Input event processing has finished processing; resources are released.
    ReadyInput(
        (
            T::Time,
            Vec<T::Input>,
            T::InputState,
            WrappedUpdateResult<T>,
        ),
    ),

    // Schedule event processing has finished processing; resources are released.
    ReadyScheduled(
        (
            Arc<InternalScheduledEvent<T>>,
            Option<T::InputState>,
            WrappedUpdateResult<T>,
        ),
    ),

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
    ) -> Self {
        let (fut, sender) = CurriedScheduleFuture::new(frame, event_arc, state);
        TransposerUpdate::Schedule((fut, sender))
    }

    pub fn poll(
        &mut self,
        poll_time: T::Time,
        input_state: Option<T::InputState>,
        cx: &mut Context<'_>,
    ) -> TransposerUpdatePoll<T> {
        if let Some(state) = input_state {
            if let Self::Schedule((_, sendrecv)) = self {
                let sendrecv = std::mem::take(sendrecv);
                if let Some((sender, receiver)) = sendrecv {
                    // don't really care about the error here
                    let _ = sender.send(state);
                }
            }
        }
        loop {
            match self {
                Self::Input(input_fut) => match input_fut.as_mut().poll(cx) {
                    Poll::Ready(result) => {
                        if let Self::Input(fut) = std::mem::take(self) {
                            let (time, inputs, in_state) = fut.recover_pinned();
                            *self = Self::ReadyInput((time, inputs, in_state, result));
                        } else {
                            unreachable!()
                        }
                    }
                    Poll::Pending => break TransposerUpdatePoll::Pending,
                },
                Self::Schedule((schedule_fut, sendrecv)) => match schedule_fut.as_mut().poll(cx) {
                    Poll::Ready(result) => {
                        if let Self::Schedule((fut, _)) = std::mem::take(self) {
                            let (event_arc, in_state) = fut.recover_pinned();
                            *self = Self::ReadyScheduled((event_arc, in_state, result));
                        } else {
                            unreachable!()
                        }
                    }
                    Poll::Pending => {
                        break if let Some((_, recv)) = sendrecv {
                            match recv.try_recv().unwrap() {
                                Some(()) => TransposerUpdatePoll::NeedsState,
                                None => TransposerUpdatePoll::Pending,
                            }
                        } else {
                            TransposerUpdatePoll::Pending
                        }
                    }
                },
                Self::ReadyInput((time, ..)) => {
                    if *time > poll_time {
                        break TransposerUpdatePoll::Scheduled(*time);
                    }

                    // this replaces self with the none variant, decomposes the current ready input, and returns it in the poll
                    if let Self::ReadyInput((time, inputs, _in_state, result)) =
                        std::mem::take(self)
                    {
                        break TransposerUpdatePoll::Ready((result, time, inputs));
                    } else {
                        unreachable!()
                    }
                }
                Self::ReadyScheduled((event_arc, ..)) => {
                    let time = event_arc.time;
                    if time > poll_time {
                        break TransposerUpdatePoll::Scheduled(time);
                    }

                    // this replaces self with the none variant, decomposes the current ready input, and returns it in the poll
                    if let Self::ReadyScheduled((_event_arc, _in_state, result)) =
                        std::mem::take(self)
                    {
                        break TransposerUpdatePoll::Ready((result, time, Vec::new()));
                    } else {
                        unreachable!()
                    }
                }
                Self::None => break TransposerUpdatePoll::Pending,
            }
        }
    }

    pub fn is_some(&self) -> bool {
        !matches!(self, Self::None)
    }

    #[allow(dead_code)]
    pub fn unstage(&mut self) -> Option<(T::Time, Vec<T::Input>, Option<T::InputState>)> {
        match std::mem::take(self) {
            Self::Input(fut) => {
                let (time, inputs, in_state) = fut.recover_pinned();
                Some((time, inputs, Some(in_state)))
            }
            Self::Schedule((fut, _send)) => {
                let (event_arc, in_state) = fut.recover_pinned();
                Some((event_arc.time, Vec::new(), in_state))
            }
            Self::ReadyInput((time, inputs, in_state, _update_result)) => {
                Some((time, inputs, Some(in_state)))
            }
            Self::ReadyScheduled((event_arc, in_state, _update_result)) => {
                Some((event_arc.time, Vec::new(), in_state))
            }
            Self::None => None,
        }
    }
}

pub(super) enum TransposerUpdatePoll<T: Transposer> {
    Ready((WrappedUpdateResult<T>, T::Time, Vec<T::Input>)),
    Scheduled(T::Time),
    NeedsState,
    Pending,
}
