use super::{curried_init_future::CurriedInitFuture, curried_input_future::CurriedInputFuture, curried_schedule_future::CurriedScheduleFuture, engine_time::{EngineTime, EngineTimeSchedule}, transposer::Transposer, transposer_frame::TransposerFrame, wrapped_update_result::UpdateResult};
use futures::{
    channel::oneshot::{Receiver, Sender},
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
    Ready{
        time: Arc<EngineTime<T::Time>>,
        inputs: Option<Vec<T::Input>>,
        input_state: Option<T::InputState>,
        result: UpdateResult<T>,
    },
    NeedsState(Sender<T::InputState>),
    Pending,
}
pub(super) enum TransposerUpdateRecovery<T: Transposer> {
    Init{
        input_state: Option<T::InputState>,
    },
    Input{
        time: T::Time,
        inputs: Vec<T::Input>,
        input_state: Option<T::InputState>,
    },
    Schedule{
        time: EngineTimeSchedule<T::Time>,
        input_state: Option<T::InputState>,
    },
}

#[allow(unused)]
impl<'a, T: Transposer> TransposerUpdate<'a, T> {
    pub fn new_init(
        transposer: T,
        state: Option<T::InputState>
    ) -> Self {
        Self(TransposerUpdateInner::new_init(transposer, state))
    }
    pub fn new_input(
        frame: TransposerFrame<T>,
        time: T::Time,
        inputs: Vec<T::Input>,
        state: Option<T::InputState>,
    ) -> Self {
        Self(TransposerUpdateInner::new_input(frame, time, inputs, state))
    }

    pub fn new_schedule(mut frame: TransposerFrame<T>, state: Option<T::InputState>) -> Self {
        if let (Some((time, payload)), new_schedule) = frame.internal.schedule.without_min_with_key() {
            frame.internal.schedule = new_schedule;
            let EngineTimeSchedule {
                time,
                parent,
                parent_index,
            } = time.as_ref().clone();
            Self(TransposerUpdateInner::new_schedule(frame, time, parent, parent_index, payload, state))
        } else {
            panic!()
        }
    }

    pub fn init_pinned(self: Pin<&mut Self>) {
        let inner: Pin<&mut TransposerUpdateInner<'a, T>> = self.project().0;
        inner.init();
    }

    pub fn init(self) -> Pin<Box<Self>> {
        let mut pinned = Box::pin(self);
        pinned.as_mut().init_pinned();
        pinned
    }

    pub fn time(&self) -> T::Time {
        self.0.time()
    }

    pub fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> TransposerUpdatePoll<T> {
        let inner = self.project().0;
        inner.poll(cx)
    }

    pub fn recover(self) -> Option<TransposerUpdateRecovery<T>> {
        match self.0 {
            TransposerUpdateInner::Init{future, state_request_receiver} => {
                let input_state = future.recover();
                Some(TransposerUpdateRecovery::Init{input_state})
            },
            TransposerUpdateInner::Input{future, state_request_receiver} => {
                let (time, inputs, input_state) = future.recover();
                Some(TransposerUpdateRecovery::Input{time, inputs, input_state})
            }
            TransposerUpdateInner::Schedule{future, state_request_receiver} => {
                let (time, input_state) = future.recover();
                Some(TransposerUpdateRecovery::Schedule{time, input_state})
            }
            TransposerUpdateInner::None => None,
        }
    }

    pub fn is_some(&self) -> bool {
        match self.0 {
            TransposerUpdateInner::None => false,
            _ => true,
        }
    }
}

#[pin_project(project = TransposerUpdateProject)]
enum TransposerUpdateInner<'a, T: Transposer> {
    // Initializing has begun; future has not returned yet.
    Init{
        #[pin]
        future: CurriedInitFuture<'a, T>,
    
        state_request_receiver: Option<Receiver<Sender<T::InputState>>>,
    },

    // Input event processing has begun; future has not returned yet.
    Input{
        #[pin]
        future: CurriedInputFuture<'a, T>,
    
        state_request_receiver: Option<Receiver<Sender<T::InputState>>>,
    },

    // Schedule event processing has begun; future has not returned yet.
    Schedule{
        #[pin]
        future: CurriedScheduleFuture<'a, T>,
    
        state_request_receiver: Option<Receiver<Sender<T::InputState>>>,
    },
    None,
}

impl<'a, T: Transposer> Default for TransposerUpdateInner<'a, T> {
    fn default() -> Self {
        TransposerUpdateInner::None
    }
}


impl<'a, T: Transposer> TransposerUpdateInner<'a, T> {
    pub fn new_init(
        transposer: T,
        state: Option<T::InputState>
    ) -> Self {
        match state {
            Some(state) => {
                let future = CurriedInitFuture::new_with_state(transposer, state);
                TransposerUpdateInner::Init{
                    future,
                    state_request_receiver: None,
                }
            }
            None => {
                let (future, state_request_receiver) = CurriedInitFuture::new(transposer);
                TransposerUpdateInner::Init{
                    future,
                    state_request_receiver: Some(state_request_receiver),
                }
            }
        }
    }
    pub fn new_input(
        frame: TransposerFrame<T>,
        time: T::Time,
        inputs: Vec<T::Input>,
        state: Option<T::InputState>,
    ) -> Self {
        match state {
            Some(state) => {
                let future = CurriedInputFuture::new_with_state(frame, time, inputs, state);
                TransposerUpdateInner::Input{
                    future,
                    state_request_receiver: None,
                }
            }
            None => {
                let (future, state_request_receiver) = CurriedInputFuture::new(frame, time, inputs);
                TransposerUpdateInner::Input{
                    future,
                    state_request_receiver: Some(state_request_receiver),
                }
            }
        }
    }

    pub fn new_schedule(
        frame: TransposerFrame<T>,
        time: T::Time,
        parent: Arc<EngineTime<T::Time>>,
        parent_index: usize,
        payload: T::Scheduled,
        state: Option<T::InputState>,
    ) -> Self {
        let time = EngineTimeSchedule {
            time,
            parent,
            parent_index,
        };

        match state {
            Some(state) => {
                let future = CurriedScheduleFuture::new_with_state(frame, time, payload, state);
                TransposerUpdateInner::Schedule{
                    future,
                    state_request_receiver: None,
                }
            }
            None => {
                let (future, state_request_receiver) = CurriedScheduleFuture::new(frame, time, payload);
                TransposerUpdateInner::Schedule{
                    future,
                    state_request_receiver: Some(state_request_receiver),
                }
            }
        }
    }

    pub fn init(self: Pin<&mut Self>) {
        match self.project() {
            TransposerUpdateProject::Init{future, ..} => future.init(),
            TransposerUpdateProject::Input{future, ..} => future.init(),
            TransposerUpdateProject::Schedule{future, ..} => future.init(),
            TransposerUpdateProject::None => {},
        }
    }

    pub fn time(&self) -> T::Time {
        match self {
            TransposerUpdateInner::Init{ .. } => T::Time::default(),
            TransposerUpdateInner::Input{future, ..} => future.time().time(),
            TransposerUpdateInner::Schedule{future, ..} => future.time().time(),
            TransposerUpdateInner::None => panic!(),
        }
    }

    pub fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> TransposerUpdatePoll<T> {
        let poll = match self.as_mut().project() {
            TransposerUpdateProject::Init{future, state_request_receiver} => {
                todo!()
            }
            TransposerUpdateProject::Input{future, state_request_receiver} => {
                let poll = future.poll(cx);

                if let Poll::Pending = poll {
                    if let Some(receiver) = state_request_receiver {
                        if let Ok(Some(sender)) = receiver.try_recv() {
                            return TransposerUpdatePoll::NeedsState(sender);
                        }
                    }
                }

                poll
            }
            TransposerUpdateProject::Schedule{future, state_request_receiver} => {
                let poll = future.poll(cx);

                if let Poll::Pending = poll {
                    if let Some(receiver) = state_request_receiver {
                        if let Ok(Some(sender)) = receiver.try_recv() {
                            return TransposerUpdatePoll::NeedsState(sender);
                        }
                    }
                }

                poll
            }
            TransposerUpdateProject::None => panic!(),
        };

        match poll {
            Poll::Ready(result) => match std::mem::take(unsafe { self.get_unchecked_mut() }) {
                Self::Init{ future, .. } => {todo!()}
                Self::Input{ future, .. } => {
                    let (time, inputs, input_state) = future.recover();
                    TransposerUpdatePoll::Ready{
                        time: EngineTime::new_input(time),
                        inputs: Some(inputs),
                        input_state,
                        result,
                    }
                }
                Self::Schedule{ future, .. } => {
                    let (time, input_state) = future.recover();
                    TransposerUpdatePoll::Ready{
                        time: time.into(),
                        inputs: None,
                        input_state,
                        result,
                    }
                }
                Self::None => unreachable!(),
            },
            Poll::Pending => TransposerUpdatePoll::Pending,
        }
    }
}
