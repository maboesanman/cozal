use core::cmp::Ordering;
use core::mem::replace;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

use futures_core::Future;

use super::args::{InitArg, InputArg, ScheduledArg};
use super::step_update_context::StepUpdateContext;
use super::time::StepTime;
use super::update::{Arg, UpdateContext, UpdateResult, WrappedTransposer, WrappedUpdate};
use crate::transposer::Transposer;
use crate::util::take_mut;

pub struct Step<T: Transposer> {
    time:            StepTime<T::Time>,
    inner:           StepInner<T>,
    outputs_emitted: OutputsEmitted,

    // these are used purely for enforcing that saturate calls use the previous wrapped_transposer.
    #[cfg(debug_assertions)]
    uuid_self: uuid::Uuid,
    #[cfg(debug_assertions)]
    uuid_prev: Option<uuid::Uuid>,
}

#[derive(Debug)]
pub enum NextUnsaturatedErr {
    NotSaturated,
    #[cfg(debug_assertions)]
    InputPastOrPresent,
}
#[derive(Debug)]
pub enum SaturateErr {
    PreviousNotSaturated,
    SelfNotUnsaturated,
    #[cfg(debug_assertions)]
    IncorrectPrevious,
}

#[derive(Debug)]
pub enum DesaturateErr {
    AlreadyUnsaturated,
}

#[derive(Debug)]
pub enum PollErr {
    NotSaturating,
}

impl<T: Transposer> Step<T> {
    pub fn new_init(transposer: T, rng_seed: [u8; 32]) -> Self {
        let time = StepTime::new_init();
        let wrapped_transposer = WrappedTransposer::new(transposer, rng_seed);
        let wrapped_transposer = Box::new(wrapped_transposer);
        let update = WrappedUpdate::new(wrapped_transposer, (), time.clone());
        let update = Box::pin(update);
        let inner = StepInner::SaturatingInit {
            update,
        };
        Step {
            time,
            inner,
            outputs_emitted: OutputsEmitted::Pending,

            #[cfg(debug_assertions)]
            uuid_self: uuid::Uuid::new_v4(),
            #[cfg(debug_assertions)]
            uuid_prev: None,
        }
    }

    pub fn next_unsaturated(
        &self,
        next_inputs: &mut Option<(T::Time, Box<[T::Input]>)>,
    ) -> Result<Option<Self>, NextUnsaturatedErr> {
        #[cfg(debug_assertions)]
        if let Some((t, _)) = next_inputs {
            if *t <= self.time().raw_time() {
                return Err(NextUnsaturatedErr::InputPastOrPresent)
            }
        }

        let wrapped_transposer = match &self.inner {
            StepInner::SaturatedInit {
                wrapped_transposer,
                ..
            } => wrapped_transposer.as_ref(),
            StepInner::SaturatedInput {
                wrapped_transposer,
                ..
            } => wrapped_transposer.as_ref(),
            StepInner::SaturatedScheduled {
                wrapped_transposer,
                ..
            } => wrapped_transposer.as_ref(),
            _ => return Err(NextUnsaturatedErr::NotSaturated),
        };
        let next_scheduled_time = wrapped_transposer.get_next_scheduled_time();

        let next_time_index = self.time.index() + 1;

        let (time, is_input) = match (&next_inputs, next_scheduled_time) {
            (None, None) => return Ok(None),
            (None, Some(t)) => (StepTime::new_scheduled(next_time_index, t.clone()), false),
            (Some((t, _)), None) => (StepTime::new_input(next_time_index, *t), true),
            (Some((t_i, _)), Some(t_s)) => match t_i.cmp(&t_s.time) {
                Ordering::Greater => (StepTime::new_scheduled(next_time_index, t_s.clone()), false),
                _ => (StepTime::new_input(next_time_index, *t_i), true),
            },
        };

        let inner = if is_input {
            StepInner::OriginalUnsaturatedInput {
                inputs: core::mem::take(next_inputs).unwrap().1,
            }
        } else {
            StepInner::OriginalUnsaturatedScheduled
        };

        let item = Step {
            time,
            inner,
            outputs_emitted: OutputsEmitted::Pending,

            #[cfg(debug_assertions)]
            uuid_self: uuid::Uuid::new_v4(),
            #[cfg(debug_assertions)]
            uuid_prev: Some(self.uuid_self),
        };

        Ok(Some(item))
    }

    // previous is expected to be the value produced this via next_unsaturated.
    pub fn saturate_take(&mut self, previous: &mut Self) -> Result<(), SaturateErr> {
        #[cfg(debug_assertions)]
        if self.uuid_prev != Some(previous.uuid_self) {
            return Err(SaturateErr::IncorrectPrevious)
        }

        if !previous.inner.is_saturated() {
            return Err(SaturateErr::PreviousNotSaturated)
        }

        if !self.inner.is_unsaturated() {
            return Err(SaturateErr::SelfNotUnsaturated)
        }

        let wrapped_transposer = take_mut::take_and_return_or_recover(
            &mut previous.inner,
            || StepInner::Unreachable,
            |prev| match prev {
                StepInner::SaturatedInit {
                    wrapped_transposer,
                } => (StepInner::UnsaturatedInit, wrapped_transposer),
                StepInner::SaturatedInput {
                    inputs,
                    wrapped_transposer,
                } => (
                    StepInner::RepeatUnsaturatedInput {
                        inputs,
                    },
                    wrapped_transposer,
                ),
                StepInner::SaturatedScheduled {
                    wrapped_transposer,
                } => (StepInner::RepeatUnsaturatedScheduled, wrapped_transposer),
                _ => unreachable!(),
            },
        );

        self.saturate_from_wrapped_transposer(wrapped_transposer);

        Ok(())
    }

    // previous is expected to be the value produced this via next_unsaturated.
    pub fn saturate_clone(&mut self, previous: &Self) -> Result<(), SaturateErr>
    where
        T: Clone,
    {
        #[cfg(debug_assertions)]
        if self.uuid_prev != Some(previous.uuid_self) {
            return Err(SaturateErr::IncorrectPrevious)
        }

        if !self.inner.is_unsaturated() {
            return Err(SaturateErr::SelfNotUnsaturated)
        }

        let wrapped_transposer = match &previous.inner {
            StepInner::SaturatedInit {
                wrapped_transposer,
                ..
            } => Ok(wrapped_transposer.clone()),
            StepInner::SaturatedInput {
                wrapped_transposer,
                ..
            } => Ok(wrapped_transposer.clone()),
            StepInner::SaturatedScheduled {
                wrapped_transposer,
            } => Ok(wrapped_transposer.clone()),
            _ => Err(SaturateErr::PreviousNotSaturated),
        }?;

        self.saturate_from_wrapped_transposer(wrapped_transposer);

        Ok(())
    }

    // panics if self is unsaturated.
    fn saturate_from_wrapped_transposer(&mut self, wrapped_transposer: Box<WrappedTransposer<T>>) {
        take_mut::take_or_recover(
            &mut self.inner,
            || StepInner::Unreachable,
            |next| match next {
                StepInner::OriginalUnsaturatedInput {
                    inputs,
                } => {
                    let update = WrappedUpdate::new(wrapped_transposer, inputs, self.time.clone());
                    StepInner::OriginalSaturatingInput {
                        update: Box::pin(update),
                    }
                },
                StepInner::RepeatUnsaturatedInput {
                    inputs,
                } => {
                    let update = WrappedUpdate::new(wrapped_transposer, inputs, self.time.clone());
                    StepInner::RepeatSaturatingInput {
                        update: Box::pin(update),
                    }
                },
                StepInner::OriginalUnsaturatedScheduled => {
                    let update = WrappedUpdate::new(wrapped_transposer, (), self.time.clone());
                    StepInner::OriginalSaturatingScheduled {
                        update: Box::pin(update),
                    }
                },
                StepInner::RepeatUnsaturatedScheduled => {
                    let update = WrappedUpdate::new(wrapped_transposer, (), self.time.clone());
                    StepInner::RepeatSaturatingScheduled {
                        update: Box::pin(update),
                    }
                },
                _ => unreachable!(),
            },
        );
    }

    pub fn desaturate(&mut self) -> Result<Option<T>, DesaturateErr> {
        take_mut::take_and_return_or_recover(&mut self.inner, StepInner::recover, |original| {
            match original {
                StepInner::OriginalSaturatingInput {
                    mut update,
                } => {
                    let inner = StepInner::OriginalUnsaturatedInput {
                        // elevate to panic, because we should be fully saturated in this situation
                        inputs: update.as_mut().reclaim().unwrap(),
                    };
                    (inner, Ok(None))
                },
                StepInner::RepeatSaturatingInput {
                    mut update,
                } => {
                    let inner = StepInner::RepeatUnsaturatedInput {
                        // elevate to panic, because we should be fully saturated in this situation
                        inputs: update.as_mut().reclaim().unwrap(),
                    };
                    (inner, Ok(None))
                },
                StepInner::OriginalSaturatingScheduled {
                    update: _,
                } => (StepInner::OriginalUnsaturatedScheduled, Ok(None)),
                StepInner::RepeatSaturatingScheduled {
                    update: _,
                } => (StepInner::RepeatUnsaturatedScheduled, Ok(None)),
                StepInner::SaturatedInit {
                    wrapped_transposer,
                } => (
                    StepInner::UnsaturatedInit,
                    Ok(Some(wrapped_transposer.transposer)),
                ),
                StepInner::SaturatedInput {
                    inputs,
                    wrapped_transposer,
                } => (
                    StepInner::RepeatUnsaturatedInput {
                        inputs,
                    },
                    Ok(Some(wrapped_transposer.transposer)),
                ),
                StepInner::SaturatedScheduled {
                    wrapped_transposer,
                } => (
                    StepInner::RepeatUnsaturatedScheduled,
                    Ok(Some(wrapped_transposer.transposer)),
                ),
                other => (other, Err(DesaturateErr::AlreadyUnsaturated)),
            }
        })
    }

    pub fn rollback(mut self) -> (bool, Option<Box<[T::Input]>>) {
        let inputs = match replace(&mut self.inner, StepInner::Unreachable) {
            StepInner::OriginalUnsaturatedInput {
                inputs,
            } => Some(inputs),
            StepInner::RepeatUnsaturatedInput {
                inputs,
            } => Some(inputs),
            StepInner::OriginalSaturatingInput {
                mut update,
            } => {
                // elevate to panic, because we should be fully saturated in this situation
                Some(update.as_mut().reclaim().unwrap())
            },
            StepInner::RepeatSaturatingInput {
                mut update,
            } => {
                // elevate to panic, because we should be fully saturated in this situation
                Some(update.as_mut().reclaim().unwrap())
            },
            StepInner::SaturatedInput {
                inputs, ..
            } => Some(inputs),
            _ => None,
        };

        (self.outputs_emitted.is_some(), inputs)
    }

    pub fn poll(&mut self, waker: Waker) -> Result<StepPoll<T>, PollErr> {
        let mut cx = Context::from_waker(&waker);

        fn handle_original_outputs<T: Transposer>(
            outputs: Vec<T::Output>,
            outputs_emitted: &mut OutputsEmitted,
        ) -> Result<StepPoll<T>, PollErr> {
            Ok(if outputs.is_empty() {
                *outputs_emitted = OutputsEmitted::None;
                StepPoll::ReadyNoOutputs
            } else {
                *outputs_emitted = OutputsEmitted::Some;
                StepPoll::ReadyOutputs(outputs)
            })
        }

        fn handle_pending<T: Transposer, C: UpdateContext<T>, A: Arg<T>>(
            update: &WrappedUpdate<T, C, A>,
        ) -> Result<StepPoll<T>, PollErr> {
            Ok({
                // this means there is an unrecoverable mismatch between update and inner.
                if update.needs_input_state().unwrap() {
                    StepPoll::NeedsState
                } else {
                    StepPoll::Pending
                }
            })
        }

        match &mut self.inner {
            StepInner::SaturatingInit {
                update,
            } => match update.as_mut().poll(&mut cx) {
                Poll::Ready(UpdateResult {
                    wrapped_transposer,
                    outputs,
                    arg: (),
                }) => {
                    self.inner = {
                        StepInner::SaturatedInit {
                            wrapped_transposer,
                        }
                    };

                    handle_original_outputs(outputs, &mut self.outputs_emitted)
                },
                Poll::Pending => handle_pending(update),
            },
            StepInner::OriginalSaturatingInput {
                update,
            } => match update.as_mut().poll(&mut cx) {
                Poll::Ready(UpdateResult {
                    wrapped_transposer,
                    outputs,
                    arg,
                }) => {
                    self.inner = {
                        StepInner::SaturatedInput {
                            wrapped_transposer,
                            inputs: arg,
                        }
                    };

                    handle_original_outputs(outputs, &mut self.outputs_emitted)
                },
                Poll::Pending => handle_pending(update),
            },

            StepInner::RepeatSaturatingInput {
                update,
            } => match update.as_mut().poll(&mut cx) {
                Poll::Ready(UpdateResult {
                    wrapped_transposer,
                    outputs: _,
                    arg,
                }) => {
                    self.inner = {
                        StepInner::SaturatedInput {
                            wrapped_transposer,
                            inputs: arg,
                        }
                    };

                    Ok(StepPoll::ReadyNoOutputs)
                },
                Poll::Pending => handle_pending(update),
            },
            StepInner::OriginalSaturatingScheduled {
                update,
            } => match update.as_mut().poll(&mut cx) {
                Poll::Ready(UpdateResult {
                    wrapped_transposer,
                    outputs,
                    arg: (),
                }) => {
                    self.inner = {
                        StepInner::SaturatedScheduled {
                            wrapped_transposer,
                        }
                    };

                    handle_original_outputs(outputs, &mut self.outputs_emitted)
                },
                Poll::Pending => handle_pending(update),
            },
            StepInner::RepeatSaturatingScheduled {
                update,
            } => match update.as_mut().poll(&mut cx) {
                Poll::Ready(UpdateResult {
                    wrapped_transposer,
                    outputs: _,
                    arg: (),
                }) => {
                    self.inner = {
                        StepInner::SaturatedScheduled {
                            wrapped_transposer,
                        }
                    };

                    Ok(StepPoll::ReadyNoOutputs)
                },
                Poll::Pending => handle_pending(update),
            },
            _ => Err(PollErr::NotSaturating),
        }
    }

    pub fn set_input_state(&mut self, state: T::InputState) -> Result<(), T::InputState> {
        match &mut self.inner {
            StepInner::SaturatingInit {
                update,
            } => update.as_mut().set_input_state(state),
            StepInner::OriginalSaturatingInput {
                update,
            } => update.as_mut().set_input_state(state),
            StepInner::OriginalSaturatingScheduled {
                update,
            } => update.as_mut().set_input_state(state),
            StepInner::RepeatSaturatingInput {
                update,
            } => update.as_mut().set_input_state(state),
            StepInner::RepeatSaturatingScheduled {
                update,
            } => update.as_mut().set_input_state(state),
            _ => Err(state),
        }
    }

    pub fn time(&self) -> &StepTime<T::Time> {
        &self.time
    }

    // this has the additional gurantee that the pointer returned lives until this is desaturated or dropped.
    // if you move self the returned pointer is still valid.
    pub fn finished_wrapped_transposer(&self) -> Option<&WrappedTransposer<T>> {
        match &self.inner {
            StepInner::SaturatedInit {
                wrapped_transposer,
            } => Some(wrapped_transposer),
            StepInner::SaturatedInput {
                wrapped_transposer,
                ..
            } => Some(wrapped_transposer),
            StepInner::SaturatedScheduled {
                wrapped_transposer,
            } => Some(wrapped_transposer),
            _ => None,
        }
    }
}

enum OutputsEmitted {
    Pending,
    None,
    Some,
}

impl OutputsEmitted {
    pub fn is_some(&self) -> bool {
        matches!(self, &OutputsEmitted::Some)
    }
}

type OriginalContext<T> = StepUpdateContext<T, Vec<<T as Transposer>::Output>>;
type RepeatContext<T> = StepUpdateContext<T, ()>;

type InitUpdate<T> = WrappedUpdate<T, OriginalContext<T>, InitArg<T>>;
type InputUpdate<T, C> = WrappedUpdate<T, C, InputArg<T>>;
type ScheduledUpdate<T, C> = WrappedUpdate<T, C, ScheduledArg<T>>;

type OriginalInputUpdate<T> = InputUpdate<T, OriginalContext<T>>;
type OriginalScheduledUpdate<T> = ScheduledUpdate<T, OriginalContext<T>>;
type RepeatInputUpdate<T> = InputUpdate<T, RepeatContext<T>>;
type RepeatScheduledUpdate<T> = ScheduledUpdate<T, RepeatContext<T>>;

enum StepInner<T: Transposer> {
    // notably this can never be rehydrated because you need the preceding wrapped_transposer
    // and there isn't one, because this is init.
    UnsaturatedInit,
    OriginalUnsaturatedInput {
        inputs: <InputArg<T> as Arg<T>>::Stored,
    },
    OriginalUnsaturatedScheduled,
    RepeatUnsaturatedInput {
        inputs: <InputArg<T> as Arg<T>>::Stored,
    },
    RepeatUnsaturatedScheduled,
    SaturatingInit {
        update: Pin<Box<InitUpdate<T>>>,
    },
    OriginalSaturatingInput {
        update: Pin<Box<OriginalInputUpdate<T>>>,
    },
    OriginalSaturatingScheduled {
        update: Pin<Box<OriginalScheduledUpdate<T>>>,
    },
    RepeatSaturatingInput {
        update: Pin<Box<RepeatInputUpdate<T>>>,
    },
    RepeatSaturatingScheduled {
        update: Pin<Box<RepeatScheduledUpdate<T>>>,
    },
    SaturatedInit {
        wrapped_transposer: Box<WrappedTransposer<T>>,
    },
    SaturatedInput {
        inputs:             <InputArg<T> as Arg<T>>::Stored,
        wrapped_transposer: Box<WrappedTransposer<T>>,
    },
    SaturatedScheduled {
        wrapped_transposer: Box<WrappedTransposer<T>>,
    },
    Unreachable,
}

impl<T: Transposer> StepInner<T> {
    fn recover() -> Self {
        Self::Unreachable
    }

    pub fn is_unsaturated(&self) -> bool {
        matches!(
            self,
            StepInner::OriginalUnsaturatedInput { .. }
                | StepInner::OriginalUnsaturatedScheduled
                | StepInner::RepeatUnsaturatedInput { .. }
                | StepInner::RepeatUnsaturatedScheduled
        )
    }

    pub fn is_saturated(&self) -> bool {
        matches!(
            self,
            StepInner::SaturatedInit { .. }
                | StepInner::SaturatedInput { .. }
                | StepInner::SaturatedScheduled { .. }
        )
    }
}

#[derive(Debug)]
pub enum StepPoll<T: Transposer> {
    Pending,
    NeedsState,
    ReadyNoOutputs,
    ReadyOutputs(Vec<T::Output>),
}
