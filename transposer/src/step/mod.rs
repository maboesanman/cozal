mod expire_handle_factory;
mod interpolate_context;
mod interpolation;
mod step_inputs;
mod sub_step_update_context;
mod time;
mod transposer_metadata;
mod wrapped_transposer;

#[cfg(test)]
mod test;

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Waker};
use std::ops::DerefMut;
use std::sync::Arc;

use futures_channel::{mpsc, oneshot};
use futures_util::{FutureExt, StreamExt};
pub use interpolation::Interpolation;
use step_inputs::StepInputs;
use time::ScheduledTime;
use wrapped_transposer::WrappedTransposer;

use crate::schedule_storage::{DefaultStorage, RefCounted, StorageFamily};
use crate::{Transposer, TransposerInput};

enum StepData<T: Transposer> {
    Init,
    Input(StepInputs<T>),
    Scheduled(ScheduledTime<T::Time>),
}

type SaturationFuture<'a, T, S> =
    Pin<Box<dyn 'a + Future<Output = <S as StorageFamily>::Transposer<WrappedTransposer<T, S>>>>>;

enum StepStatus<T: Transposer, S: StorageFamily> {
    Unsaturated,
    Saturating {
        future:          SaturationFuture<'static, T, S>,
        output_reciever: mpsc::Receiver<(T::OutputEvent, oneshot::Sender<()>)>,
    },
    Saturated {
        wrapped_transposer: S::Transposer<WrappedTransposer<T, S>>,
    },
}

impl<T: Transposer, S: StorageFamily> Default for StepStatus<T, S> {
    fn default() -> Self {
        Self::Unsaturated
    }
}

pub struct Step<T: Transposer, Is: InputState<T>, S: StorageFamily = DefaultStorage> {
    data:        Arc<StepData<T>>,
    input_state: S::LazyState<Is>,
    status:      StepStatus<T, S>,
    event_count: usize,

    #[cfg(debug_assertions)]
    uuid_self: uuid::Uuid,
    #[cfg(debug_assertions)]
    uuid_prev: Option<uuid::Uuid>,
}

/// this type holds the lazy state values for all inputs.
/// all the lazy population logic is left to the instantiator of step.
pub trait InputState<T: Transposer> {
    fn new() -> Self;
    fn get_provider(&self) -> &T::InputStateManager;
}

pub struct NoInput;
pub struct NoInputManager;

impl<T: Transposer<InputStateManager = NoInputManager>> InputState<T> for NoInput {
    fn new() -> Self {
        NoInput
    }

    fn get_provider(&self) -> &<T as Transposer>::InputStateManager {
        &NoInputManager
    }
}

impl<T: Transposer, Is: InputState<T>, S: StorageFamily> Drop for Step<T, Is, S> {
    fn drop(&mut self) {
        let status = core::mem::replace(&mut self.status, StepStatus::Unsaturated);

        match status {
            StepStatus::Unsaturated => {},
            StepStatus::Saturating {
                future,
                output_reciever: _,
            } => {
                let future: SaturationFuture<'static, T, S> = future;
                // SAFETY: the future here can only hold things that the step is already generic over and contains.
                // this means that this lifetime forging to 'static is ok.
                let future: SaturationFuture<'_, T, S> = unsafe { core::mem::transmute(future) };

                drop(future);
            },
            StepStatus::Saturated {
                wrapped_transposer: _,
            } => {},
        }
    }
}

impl<T: Transposer, Is: InputState<T>, S: StorageFamily> Step<T, Is, S> {
    pub fn new_init(transposer: T, rng_seed: [u8; 32]) -> Self {
        let input_state = S::LazyState::new(Box::new(Is::new()));
        let (output_sender, output_reciever) = mpsc::channel(1);
        let future = WrappedTransposer::<T, S>::init::<Is>(
            transposer,
            rng_seed,
            input_state.clone(),
            0,
            output_sender,
        );
        let future: SaturationFuture<'_, T, S> = Box::pin(future);
        // SAFETY: the future here can only hold things that the step is already generic over and contains.
        // this means that this lifetime forging to 'static is ok.
        let future: SaturationFuture<'static, T, S> = unsafe { core::mem::transmute(future) };

        let status = StepStatus::Saturating {
            future,
            output_reciever,
        };

        Step {
            data: Arc::new(StepData::Init),
            input_state,
            status,
            event_count: 0,

            #[cfg(debug_assertions)]
            uuid_self: uuid::Uuid::new_v4(),
            #[cfg(debug_assertions)]
            uuid_prev: None,
        }
    }

    pub fn next_unsaturated<I: TransposerInput<Base = T>>(
        &self,
        next_inputs: &mut Option<StepInputs<T>>,
    ) -> Result<Option<Self>, NextUnsaturatedErr> {
        let wrapped_transposer = match &self.status {
            StepStatus::Saturated {
                wrapped_transposer,
            } => wrapped_transposer,
            _ => return Err(NextUnsaturatedErr::NotSaturated),
        };

        let next_scheduled_time = wrapped_transposer.metadata.get_next_scheduled_time();
        let next_inputs_time = next_inputs.as_ref().map(|i| i.time);
        let data = match (next_inputs_time, next_scheduled_time) {
            (None, None) => return Ok(None),
            (None, Some(t)) => StepData::Scheduled(*t),
            (Some(_), None) => StepData::Input(core::mem::take(next_inputs).unwrap()),
            (Some(i_t), Some(s_t)) => {
                if i_t > s_t.time {
                    StepData::Scheduled(*s_t)
                } else {
                    StepData::Input(core::mem::take(next_inputs).unwrap())
                }
            },
        };

        Ok(Some(Self {
            data:        Arc::new(data),
            input_state: S::LazyState::new(Box::new(Is::new())),
            status:      StepStatus::Unsaturated,
            event_count: 0,

            #[cfg(debug_assertions)]
            uuid_self:                          uuid::Uuid::new_v4(),
            #[cfg(debug_assertions)]
            uuid_prev:                          Some(self.uuid_self),
        }))
    }

    pub fn next_scheduled_unsaturated(&self) -> Result<Option<Self>, NextUnsaturatedErr> {
        let wrapped_transposer = match &self.status {
            StepStatus::Saturated {
                wrapped_transposer,
            } => wrapped_transposer,
            _ => return Err(NextUnsaturatedErr::NotSaturated),
        };

        let next_scheduled_time = wrapped_transposer.metadata.get_next_scheduled_time();
        let data = match next_scheduled_time {
            None => return Ok(None),
            Some(t) => StepData::Scheduled(*t),
        };

        Ok(Some(Self {
            data:        Arc::new(data),
            input_state: S::LazyState::new(Box::new(Is::new())),
            status:      StepStatus::Unsaturated,
            event_count: 0,

            #[cfg(debug_assertions)]
            uuid_self:                          uuid::Uuid::new_v4(),
            #[cfg(debug_assertions)]
            uuid_prev:                          Some(self.uuid_self),
        }))
    }

    pub fn saturate_take(&mut self, prev: &mut Self) -> Result<(), SaturateTakeErr> {
        #[cfg(debug_assertions)]
        if self.uuid_prev != Some(prev.uuid_self) {
            return Err(SaturateTakeErr::IncorrectPrevious)
        }

        let wrapped_transposer = prev.take()?;

        self.saturate(wrapped_transposer);

        Ok(())
    }

    pub fn saturate_clone(&mut self, prev: &Self) -> Result<(), SaturateCloneErr>
    where
        T: Clone,
    {
        #[cfg(debug_assertions)]
        if self.uuid_prev != Some(prev.uuid_self) {
            return Err(SaturateCloneErr::IncorrectPrevious)
        }

        let wrapped_transposer = prev.clone()?;

        self.saturate(wrapped_transposer);

        Ok(())
    }

    fn take(&mut self) -> Result<S::Transposer<WrappedTransposer<T, S>>, SaturateTakeErr> {
        match core::mem::take(&mut self.status) {
            StepStatus::Saturated {
                wrapped_transposer,
            } => Ok(wrapped_transposer),
            val => {
                self.status = val;
                Err(SaturateTakeErr::PreviousNotSaturated)
            },
        }
    }

    fn clone(&self) -> Result<S::Transposer<WrappedTransposer<T, S>>, SaturateCloneErr>
    where
        T: Clone,
    {
        match &self.status {
            StepStatus::Saturated {
                wrapped_transposer,
            } => Ok(S::Transposer::new(Box::new(WrappedTransposer::clone(
                wrapped_transposer,
            )))),
            _ => Err(SaturateCloneErr::PreviousNotSaturated),
        }
    }

    fn saturate(&mut self, mut wrapped_transposer: S::Transposer<WrappedTransposer<T, S>>) {
        let (output_sender, output_reciever) = mpsc::channel(1);

        self.status = StepStatus::Saturating {
            future: match self.data.as_ref() {
                StepData::Init => panic!(),
                StepData::Input(_) => {
                    let input_state = self.input_state.clone();
                    let event_count = self.event_count;
                    let step_data = self.data.clone();
                    let future: SaturationFuture<'_, T, S> = Box::pin(async move {
                        let i = match step_data.as_ref() {
                            StepData::Input(i) => i,
                            _ => unreachable!(),
                        };
                        wrapped_transposer
                            .mutate()
                            .handle_input(i, input_state, event_count, output_sender)
                            .await;
                        wrapped_transposer
                    });
                    // SAFETY: the future here can only hold things that the step is already generic over and contains.
                    // this means that this lifetime forging to 'static is ok.
                    let future: SaturationFuture<'static, T, S> =
                        unsafe { core::mem::transmute(future) };
                    future
                },
                StepData::Scheduled(t) => {
                    let t = t.time;
                    let event_count = self.event_count;
                    let input_state = self.input_state.clone();
                    let future: SaturationFuture<'_, T, S> = Box::pin(async move {
                        wrapped_transposer
                            .mutate()
                            .handle_scheduled(t, input_state, event_count, output_sender)
                            .await;
                        wrapped_transposer
                    });
                    // SAFETY: the future here can only hold things that the step is already generic over and contains.
                    // this means that this lifetime forging to 'static is ok.
                    let future: SaturationFuture<'static, T, S> =
                        unsafe { core::mem::transmute(future) };
                    future
                },
            },
            output_reciever,
        };
    }

    pub fn desaturate(&mut self) {
        self.status = StepStatus::Unsaturated;
        self.input_state = S::LazyState::new(Box::new(Is::new()));
    }

    pub fn poll(&mut self, waker: &Waker) -> Result<StepPoll<T>, PollErr> {
        let (future, output_reciever) = match &mut self.status {
            StepStatus::Unsaturated => return Err(PollErr::Unsaturated),
            StepStatus::Saturating {
                future,
                output_reciever,
            } => (future, output_reciever),
            StepStatus::Saturated {
                ..
            } => return Err(PollErr::Saturated),
        };

        let mut cx = Context::from_waker(waker);

        let poll = future.poll_unpin(&mut cx);

        let output = match poll {
            std::task::Poll::Ready(wrapped_transposer) => {
                self.status = StepStatus::Saturated {
                    wrapped_transposer,
                };
                return Ok(StepPoll::Ready)
            },
            std::task::Poll::Pending => output_reciever.poll_next_unpin(&mut cx),
        };

        if let std::task::Poll::Ready(Some((e, sender))) = output {
            self.event_count += 1;
            let _ = sender.send(());
            return Ok(StepPoll::Emitted(e))
        }

        Ok(StepPoll::Pending)
    }

    pub fn interpolate(&self, time: T::Time) -> Result<Interpolation<T, Is, S>, InterpolateErr> {
        let wrapped_transposer = match &self.status {
            StepStatus::Saturated {
                wrapped_transposer,
            } => wrapped_transposer.clone(),
            _ => return Err(InterpolateErr::NotSaturated),
        };

        if time < wrapped_transposer.metadata.last_updated.time {
            return Err(InterpolateErr::TimePast)
        }

        Ok(Interpolation::new(time, wrapped_transposer))
    }

    pub fn get_input_state(&self) -> &Is {
        &self.input_state
    }

    pub fn get_time(&self) -> T::Time {
        match self.data.as_ref() {
            StepData::Init => T::Time::default(),
            StepData::Input(i) => i.time,
            StepData::Scheduled(t) => t.time,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum StepPoll<T: Transposer> {
    Emitted(T::OutputEvent),
    Pending,
    Ready,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PollErr {
    Unsaturated,
    Saturated,
}

#[derive(Debug)]
pub enum InterpolateErr {
    NotSaturated,
    #[cfg(debug_assertions)]
    TimePast,
}

#[derive(Debug)]
pub enum NextUnsaturatedErr {
    NotSaturated,
    #[cfg(debug_assertions)]
    InputPastOrPresent,
}

#[derive(Debug)]
pub enum SaturateTakeErr {
    PreviousNotSaturated,
    SelfNotUnsaturated,
    #[cfg(debug_assertions)]
    IncorrectPrevious,
    PreviousHasActiveInterpolations,
}

#[derive(Debug)]
pub enum SaturateCloneErr {
    PreviousNotSaturated,
    SelfNotUnsaturated,
    #[cfg(debug_assertions)]
    IncorrectPrevious,
}
