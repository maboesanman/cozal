use super::{
    curried_input_future::CurriedInputFuture, curried_schedule_future::CurriedScheduleFuture,
    internal_scheduled_event::InternalScheduledEvent, transposer::Transposer,
    transposer_frame::TransposerFrame, wrapped_update_result::WrappedUpdateResult,
};
use futures::{
    channel::oneshot::{channel, Receiver, Sender},
    Future,
};
use pin_project::pin_project;
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

#[pin_project]
pub(super) struct TransposerUpdate<'a, T: Transposer>(#[pin] TransposerUpdateInner<'a, T>);
pub(super) enum TransposerUpdatePoll<T: Transposer> {
    Ready(ReadyResult<T>),
    NeedsState,
    Pending,
}
pub(super) enum TransposerUpdateRecovery<T: Transposer> {
    Input(Vec<T::Input>, T::InputState),
    Schedule(Arc<InternalScheduledEvent<T>>, Option<T::InputState>)
}

#[allow(unused)]
impl<'a, T: Transposer> TransposerUpdate<'a, T> {
    pub fn new_input(
        frame: TransposerFrame<T>,
        time: T::Time,
        inputs: Vec<T::Input>,
        state: T::InputState,
    ) -> Self {
        Self(TransposerUpdateInner::new_input(frame, time, inputs, state))
    }

    pub fn new_schedule(
        frame: TransposerFrame<T>,
        event_arc: Arc<InternalScheduledEvent<T>>,
        state: Option<T::InputState>,
    ) -> Self {
        Self(TransposerUpdateInner::new_schedule(frame, event_arc, state))
    }

    pub fn init_pinned(self: Pin<&mut Self>) {
        let inner = self.project().0;
        inner.init();
    }

    pub fn init(self) -> Pin<Box<Self>> {
        let mut pinned = Box::pin(self);
        pinned.as_mut().init_pinned();
        pinned
    }

    pub fn time(self: Pin<&Self>) -> T::Time {
        self.get_ref().0.time()
    }

    pub fn poll(
        self: Pin<&mut Self>,
        input_state: Option<T::InputState>,
        cx: &mut Context<'_>,
    ) -> TransposerUpdatePoll<T> {
        let inner = self.project().0;
        inner.poll(input_state, cx)
    }

    pub fn recover(self) -> TransposerUpdateRecovery<T> {
        match self.0 {
            TransposerUpdateInner::Input(update_input) => {
                let (time, inputs, state) = update_input.future.recover();
                TransposerUpdateRecovery::Input(inputs, state)
            }
            TransposerUpdateInner::Schedule(update_schedule) => {
                let (event_arc, state) = update_schedule.future.recover();
                TransposerUpdateRecovery::Schedule(event_arc, state)
            }
            TransposerUpdateInner::Done => panic!()
        }
    }
}

#[pin_project(project = TransposerUpdateProject)]
enum TransposerUpdateInner<'a, T: Transposer> {
    // Input event processing has begun; future has not returned yet.
    Input(#[pin] UpdateInput<'a, T>),

    // Schedule event processing has begun; future has not returned yet.
    Schedule(#[pin] UpdateSchedule<'a, T>),

    Done,
}


impl<'a, T: Transposer> Default for TransposerUpdateInner<'a, T> {
    fn default() -> Self {
        TransposerUpdateInner::Done
    }
}

#[pin_project]
struct UpdateInput<'a, T: Transposer> {
    #[pin]
    future: CurriedInputFuture<'a, T>,
}

#[pin_project]
struct UpdateSchedule<'a, T: Transposer> {
    #[pin]
    future: CurriedScheduleFuture<'a, T>,

    state_notify: StateNotify<T::InputState>,
}

enum StateNotify<S> {
    Complete {
        state_sender: Sender<S>,
        notification_receiver: Receiver<()>,
    },
    Incomplete {
        state_sender: Sender<S>,
    },
    None,
}

impl<S> StateNotify<S> {
    pub fn try_send(&mut self, state: Option<S>) -> Result<(), S> {
        match state {
            Some(state) => match std::mem::take(self) {
                StateNotify::Complete { state_sender, .. } => state_sender.send(state),
                StateNotify::Incomplete { state_sender } => state_sender.send(state),
                StateNotify::None => Err(state),
            },
            None => Ok(()),
        }
    }
}

impl<S> Default for StateNotify<S> {
    fn default() -> Self {
        StateNotify::None
    }
}

pub struct ReadyResult<T: Transposer> {
    pub time: T::Time,
    pub inputs: Option<Vec<T::Input>>,
    pub input_state: Option<T::InputState>,
    pub(super) result: WrappedUpdateResult<T>,
}

impl<'a, T: Transposer> TransposerUpdateInner<'a, T> {
    pub fn new_input(
        frame: TransposerFrame<T>,
        time: T::Time,
        inputs: Vec<T::Input>,
        state: T::InputState,
    ) -> Self {
        let future = CurriedInputFuture::new(frame, time, inputs, state);
        let update_input = UpdateInput { future };
        TransposerUpdateInner::Input(update_input)
    }

    pub fn new_schedule(
        frame: TransposerFrame<T>,
        event_arc: Arc<InternalScheduledEvent<T>>,
        state: Option<T::InputState>,
    ) -> Self {
        let (future, state_notify) = CurriedScheduleFuture::new(frame, event_arc, state);
        let state_notify = match state_notify {
            Some(state_sender) => StateNotify::Incomplete { state_sender },
            None => StateNotify::None,
        };
        let update_schedule = UpdateSchedule {
            future,
            state_notify,
        };
        TransposerUpdateInner::Schedule(update_schedule)
    }

    pub fn init(self: Pin<&mut Self>) {
        match self.project() {
            TransposerUpdateProject::Input(update_input) => {
                let update_input = update_input.project();
                update_input.future.init();
            }
            TransposerUpdateProject::Schedule(update_schedule) => {
                let update_schedule = update_schedule.project();
                let prev_state_notify = std::mem::take(update_schedule.state_notify);
                let notification_reciever = match prev_state_notify {
                    StateNotify::Complete { .. } => panic!(),
                    StateNotify::Incomplete { state_sender } => {
                        let (notification_sender, notification_receiver) = channel();
                        *update_schedule.state_notify = StateNotify::Complete {
                            state_sender,
                            notification_receiver,
                        };
                        Some(notification_sender)
                    }
                    StateNotify::None => None,
                };
                update_schedule.future.init(notification_reciever);
            }
            _ => {}
        }
    }

    pub fn time(&self) -> T::Time {
        match self {
            TransposerUpdateInner::Input(update_input) => update_input.future.time(),
            TransposerUpdateInner::Schedule(update_schedule) => update_schedule.future.time(),
            TransposerUpdateInner::Done => panic!(),
        }
    }

    pub fn poll(
        mut self: Pin<&mut Self>,
        input_state: Option<T::InputState>,
        cx: &mut Context<'_>,
    ) -> TransposerUpdatePoll<T> {
        let this = self.as_mut().project();
        // if let TransposerUpdateProject::Schedule(update_schedule) = this {
        //     let update_schedule = update_schedule.project();
        //     update_schedule.state_notify.try_send(input_state);
        // }

        let result = match this {
            TransposerUpdateProject::Input(update_input) => {
                let update_input = update_input.project();
                update_input.future.poll(cx)
            }
            TransposerUpdateProject::Schedule(update_schedule) => {
                let update_schedule = update_schedule.project();
                let _ = update_schedule.state_notify.try_send(input_state);
                let result = update_schedule.future.poll(cx);

                if let Poll::Pending = result {
                    if let StateNotify::Complete {
                        notification_receiver,
                        ..
                    } = update_schedule.state_notify
                    {
                        match notification_receiver.try_recv().unwrap() {
                            Some(()) => return TransposerUpdatePoll::NeedsState,
                            None => return TransposerUpdatePoll::Pending,
                        }
                    }
                }

                result
            }
            TransposerUpdateProject::Done => panic!(),
        };

        match result {
            Poll::Ready(result) => {
                let this = std::mem::take(unsafe { self.get_unchecked_mut() });
                match this {
                    Self::Input(update_input) => {
                        let (time, inputs, input_state) = update_input.future.recover();
                        TransposerUpdatePoll::Ready(ReadyResult {
                            time,
                            inputs: Some(inputs),
                            input_state: Some(input_state),
                            result,
                        })
                    }
                    Self::Schedule(update_schedule) => {
                        let (event_arc, input_state) = update_schedule.future.recover();
                        TransposerUpdatePoll::Ready(ReadyResult {
                            time: event_arc.time,
                            inputs: None,
                            input_state,
                            result,
                        })
                    }
                    Self::Done => unreachable!(),
                }
            }
            Poll::Pending => TransposerUpdatePoll::Pending,
        }
    }
}
