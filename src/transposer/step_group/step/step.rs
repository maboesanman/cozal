use core::cmp::Ordering;
use core::pin::Pin;
use core::task::{Context, Poll};

use futures_core::Future;

use super::args::{InitArg, InputArg, ScheduledArg};
use super::step_update_context::StepUpdateContext;
use super::time::StepTime;
use super::update::{Arg, Update, UpdateResult, WrappedTransposer};
use crate::transposer::schedule_storage::StorageFamily;
use crate::transposer::step_group::lazy_state::LazyState;
use crate::transposer::step_group::NextInputs;
use crate::transposer::Transposer;
use crate::util::take_mut::{self, take_and_return_or_recover};

pub struct Step<T: Transposer, S: StorageFamily> {
    time:        StepTime<T::Time>,
    inner:       StepInner<T, S>,
    input_state: *const LazyState<T::InputState>,

    // these are used purely for enforcing that saturate calls use the previous step.
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
    Unsaturated,
    Saturated,
}

impl<T: Transposer, S: StorageFamily> Step<T, S> {
    // SAFETY: input_state must outlive returned value and all values created from `next_unsaturated_same_time`
    pub unsafe fn new_init(
        transposer: T,
        rng_seed: [u8; 32],
        input_state: *const LazyState<T::InputState>,
    ) -> Self {
        let time = StepTime::new_init();
        let wrapped_transposer = WrappedTransposer::new(transposer, rng_seed);
        let wrapped_transposer = Box::new(wrapped_transposer);
        let update = unsafe { Update::new(wrapped_transposer, (), time.clone(), input_state) };
        let inner = StepInner::SaturatingInit {
            update,
        };
        Step {
            time,
            inner,
            input_state,

            #[cfg(debug_assertions)]
            uuid_self: uuid::Uuid::new_v4(),
            #[cfg(debug_assertions)]
            uuid_prev: None,
        }
    }

    pub fn next_unsaturated(
        &self,
        next_inputs: &mut NextInputs<T>,
        input_state: *const LazyState<T::InputState>,
    ) -> Result<Option<Self>, NextUnsaturatedErr> {
        #[cfg(debug_assertions)]
        if let Some((t, _)) = next_inputs {
            let self_time = self.time().raw_time();
            if *t < self_time {
                return Err(NextUnsaturatedErr::InputPastOrPresent)
            }
            if *t == self_time && self.time().index() != 0 {
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
            input_state,

            #[cfg(debug_assertions)]
            uuid_self: uuid::Uuid::new_v4(),
            #[cfg(debug_assertions)]
            uuid_prev: Some(self.uuid_self),
        };

        Ok(Some(item))
    }

    pub fn next_unsaturated_same_time(&self) -> Result<Option<Self>, NextUnsaturatedErr> {
        // init is always its own time.
        if self.time().index() == 0 {
            return Ok(None)
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
        let next_scheduled_time = match next_scheduled_time {
            Some(t) => t,
            None => return Ok(None),
        };

        if self.time().raw_time() != next_scheduled_time.time {
            return Ok(None)
        }

        let next_time_index = self.time.index() + 1;
        let time = StepTime::new_scheduled(next_time_index, next_scheduled_time.clone());
        let inner = StepInner::OriginalUnsaturatedScheduled;

        let item = Step {
            time,
            inner,
            input_state: self.input_state,

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
    fn saturate_from_wrapped_transposer(
        &mut self,
        wrapped_transposer: Box<WrappedTransposer<T, S>>,
    ) {
        take_mut::take_or_recover(
            &mut self.inner,
            || StepInner::Unreachable,
            |next| match next {
                StepInner::OriginalUnsaturatedInput {
                    inputs,
                } => {
                    let update = unsafe {
                        Update::new(
                            wrapped_transposer,
                            inputs,
                            self.time.clone(),
                            self.input_state,
                        )
                    };
                    StepInner::OriginalSaturatingInput {
                        update,
                    }
                },
                StepInner::RepeatUnsaturatedInput {
                    inputs,
                } => {
                    let update = unsafe {
                        Update::new(
                            wrapped_transposer,
                            inputs,
                            self.time.clone(),
                            self.input_state,
                        )
                    };
                    StepInner::RepeatSaturatingInput {
                        update,
                    }
                },
                StepInner::OriginalUnsaturatedScheduled => {
                    let update = unsafe {
                        Update::new(wrapped_transposer, (), self.time.clone(), self.input_state)
                    };
                    StepInner::OriginalSaturatingScheduled {
                        update,
                    }
                },
                StepInner::RepeatUnsaturatedScheduled => {
                    let update = unsafe {
                        Update::new(wrapped_transposer, (), self.time.clone(), self.input_state)
                    };
                    StepInner::RepeatSaturatingScheduled {
                        update,
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
                    update,
                } => {
                    let inner = StepInner::OriginalUnsaturatedInput {
                        // elevate to panic, because we should be fully saturated in this situation
                        inputs: update.reclaim_pending(),
                    };
                    (inner, Ok(None))
                },
                StepInner::RepeatSaturatingInput {
                    update,
                } => {
                    let inner = StepInner::RepeatUnsaturatedInput {
                        // elevate to panic, because we should be fully saturated in this situation
                        inputs: update.reclaim_pending(),
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

    pub fn time(&self) -> &StepTime<T::Time> {
        &self.time
    }

    // this has the additional gurantee that the pointer returned lives until this is desaturated or dropped.
    // if you move self the returned pointer is still valid.
    pub fn finished_wrapped_transposer(&self) -> Option<&WrappedTransposer<T, S>> {
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

impl<T: Transposer, S: StorageFamily> Future for Step<T, S> {
    type Output = Result<Option<Vec<T::Output>>, PollErr>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        fn handle_original_outputs<O>(outputs: Vec<O>) -> Poll<Result<Option<Vec<O>>, PollErr>> {
            Poll::Ready(Ok(if outputs.is_empty() {
                None
            } else {
                Some(outputs)
            }))
        }

        take_and_return_or_recover(
            &mut self.inner,
            || StepInner::Unreachable,
            |inner| match inner {
                StepInner::SaturatingInit {
                    mut update,
                } => match Pin::new(&mut update).poll(cx) {
                    Poll::Ready(()) => {
                        let UpdateResult {
                            wrapped_transposer,
                            outputs,
                            arg: (),
                        } = unsafe { update.reclaim_ready() };
                        let inner = StepInner::SaturatedInit {
                            wrapped_transposer,
                        };

                        (inner, handle_original_outputs(outputs))
                    },
                    Poll::Pending => (
                        StepInner::SaturatingInit {
                            update,
                        },
                        Poll::Pending,
                    ),
                },
                StepInner::OriginalSaturatingInput {
                    mut update,
                } => match Pin::new(&mut update).poll(cx) {
                    Poll::Ready(()) => {
                        let UpdateResult {
                            wrapped_transposer,
                            outputs,
                            arg,
                        } = unsafe { update.reclaim_ready() };
                        let inner = StepInner::SaturatedInput {
                            wrapped_transposer,
                            inputs: arg,
                        };

                        (inner, handle_original_outputs(outputs))
                    },
                    Poll::Pending => (
                        StepInner::OriginalSaturatingInput {
                            update,
                        },
                        Poll::Pending,
                    ),
                },

                StepInner::RepeatSaturatingInput {
                    mut update,
                } => match Pin::new(&mut update).poll(cx) {
                    Poll::Ready(()) => {
                        let UpdateResult {
                            wrapped_transposer,
                            outputs: (),
                            arg,
                        } = unsafe { update.reclaim_ready() };
                        let inner = StepInner::SaturatedInput {
                            wrapped_transposer,
                            inputs: arg,
                        };

                        (inner, Poll::Ready(Ok(None)))
                    },
                    Poll::Pending => (
                        StepInner::RepeatSaturatingInput {
                            update,
                        },
                        Poll::Pending,
                    ),
                },
                StepInner::OriginalSaturatingScheduled {
                    mut update,
                } => match Pin::new(&mut update).poll(cx) {
                    Poll::Ready(()) => {
                        let UpdateResult {
                            wrapped_transposer,
                            outputs,
                            arg: (),
                        } = unsafe { update.reclaim_ready() };
                        let inner = StepInner::SaturatedScheduled {
                            wrapped_transposer,
                        };

                        (inner, handle_original_outputs(outputs))
                    },
                    Poll::Pending => (
                        StepInner::OriginalSaturatingScheduled {
                            update,
                        },
                        Poll::Pending,
                    ),
                },
                StepInner::RepeatSaturatingScheduled {
                    mut update,
                } => match Pin::new(&mut update).poll(cx) {
                    Poll::Ready(()) => {
                        let UpdateResult {
                            wrapped_transposer,
                            outputs: (),
                            arg: (),
                        } = unsafe { update.reclaim_ready() };
                        let inner = StepInner::SaturatedScheduled {
                            wrapped_transposer,
                        };

                        (inner, Poll::Ready(Ok(None)))
                    },
                    Poll::Pending => (
                        StepInner::RepeatSaturatingScheduled {
                            update,
                        },
                        Poll::Pending,
                    ),
                },
                StepInner::UnsaturatedInit
                | StepInner::OriginalUnsaturatedInput {
                    ..
                }
                | StepInner::OriginalUnsaturatedScheduled
                | StepInner::RepeatUnsaturatedInput {
                    ..
                }
                | StepInner::RepeatUnsaturatedScheduled => {
                    (inner, Poll::Ready(Err(PollErr::Unsaturated)))
                },
                StepInner::SaturatedInit {
                    ..
                }
                | StepInner::SaturatedInput {
                    ..
                }
                | StepInner::SaturatedScheduled {
                    ..
                } => (inner, Poll::Ready(Err(PollErr::Saturated))),
                StepInner::Unreachable => unreachable!(),
            },
        )
    }
}

// context types
type OriginalContext<T, S> = StepUpdateContext<T, S, Vec<<T as Transposer>::Output>>;
type RepeatContext<T, S> = StepUpdateContext<T, S, ()>;

// arg types
type InitUpdate<T, S> = Update<T, S, OriginalContext<T, S>, InitArg<T, S>>;
type InputUpdate<T, S, C> = Update<T, S, C, InputArg<T, S>>;
type ScheduledUpdate<T, S, C> = Update<T, S, C, ScheduledArg<T, S>>;

// arg + context types
type OriginalInputUpdate<T, S> = InputUpdate<T, S, OriginalContext<T, S>>;
type OriginalScheduledUpdate<T, S> = ScheduledUpdate<T, S, OriginalContext<T, S>>;
type RepeatInputUpdate<T, S> = InputUpdate<T, S, RepeatContext<T, S>>;
type RepeatScheduledUpdate<T, S> = ScheduledUpdate<T, S, RepeatContext<T, S>>;

enum StepInner<T: Transposer, S: StorageFamily> {
    // notably this can never be rehydrated because you need the preceding wrapped_transposer
    // and there isn't one, because this is init.
    UnsaturatedInit,
    OriginalUnsaturatedInput {
        inputs: <InputArg<T, S> as Arg<T, S>>::Stored,
    },
    OriginalUnsaturatedScheduled,
    RepeatUnsaturatedInput {
        inputs: <InputArg<T, S> as Arg<T, S>>::Stored,
    },
    RepeatUnsaturatedScheduled,
    SaturatingInit {
        update: InitUpdate<T, S>,
    },
    OriginalSaturatingInput {
        update: OriginalInputUpdate<T, S>,
    },
    OriginalSaturatingScheduled {
        update: OriginalScheduledUpdate<T, S>,
    },
    RepeatSaturatingInput {
        update: RepeatInputUpdate<T, S>,
    },
    RepeatSaturatingScheduled {
        update: RepeatScheduledUpdate<T, S>,
    },
    SaturatedInit {
        wrapped_transposer: Box<WrappedTransposer<T, S>>,
    },
    SaturatedInput {
        inputs:             <InputArg<T, S> as Arg<T, S>>::Stored,
        wrapped_transposer: Box<WrappedTransposer<T, S>>,
    },
    SaturatedScheduled {
        wrapped_transposer: Box<WrappedTransposer<T, S>>,
    },
    Unreachable,
}

impl<T: Transposer, S: StorageFamily> StepInner<T, S> {
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
