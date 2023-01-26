use core::cmp::Ordering;
use core::pin::Pin;
use core::task::Context;

use util::replace_mut;

use super::args::{InitArg, InputArg, ScheduledArg};
use super::sub_step_update_context::SubStepUpdateContextFamily;
use super::time::SubStepTime;
use super::update::{Update, UpdateContext, UpdatePoll};
use super::WrappedTransposer;
use crate::schedule_storage::{RefCounted, StorageFamily};
use crate::step::step::InputState;
use crate::step::step_inputs::StepInputs;
use crate::Transposer;

pub struct SubStep<'almost_static, T: Transposer, Is: InputState<T>, S: StorageFamily>
where
    (T, Is): 'almost_static,
{
    pub time: SubStepTime<T::Time>,
    inner:    SubStepInner<'almost_static, T, S, Is>,

    // these are used purely for enforcing that saturate calls use the previous step.
    #[cfg(debug_assertions)]
    uuid_self: uuid::Uuid,
    #[cfg(debug_assertions)]
    uuid_prev: Option<uuid::Uuid>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum NextUnsaturatedErr {
    NotSaturated,
    #[cfg(debug_assertions)]
    InputPastOrPresent,
}
#[derive(Debug, PartialEq, Eq)]
pub enum SaturateErr {
    PreviousNotSaturated,
    SelfNotUnsaturated,
    #[cfg(debug_assertions)]
    IncorrectPrevious,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DesaturateErr {
    AlreadyUnsaturated,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PollErr {
    Unsaturated,
    Saturated,
}

impl<'almost_static, T: Transposer, Is: InputState<T>, S: StorageFamily>
    SubStep<'almost_static, T, Is, S>
{
    pub fn new_init(transposer: T, rng_seed: [u8; 32], input_state: S::LazyState<Is>) -> Self {
        let time = SubStepTime::new_init();
        let wrapped_transposer = WrappedTransposer::new(transposer, rng_seed);
        let wrapped_transposer = Box::new(wrapped_transposer);
        let wrapped_transposer = S::Transposer::<WrappedTransposer<T, S>>::new(wrapped_transposer);

        let update = Update::new::<SubStepUpdateContextFamily<T, S>>(
            wrapped_transposer,
            InitArg::new(),
            time.clone(),
            input_state,
        );

        let inner = SubStepInner::SaturatingInit {
            update,
        };

        SubStep {
            time,
            inner,

            #[cfg(debug_assertions)]
            uuid_self: uuid::Uuid::new_v4(),
            #[cfg(debug_assertions)]
            uuid_prev: None,
        }
    }

    pub fn next_unsaturated(
        &self,
        next_inputs: &mut Option<StepInputs<T>>,
        input_state: S::LazyState<Is>,
    ) -> Result<Option<Self>, NextUnsaturatedErr> {
        #[cfg(debug_assertions)]
        if let Some(inputs) = next_inputs {
            let self_time = self.time().raw_time();
            if inputs.time() < self_time {
                return Err(NextUnsaturatedErr::InputPastOrPresent)
            }
            if inputs.time() == self_time && self.time().index() != 0 {
                return Err(NextUnsaturatedErr::InputPastOrPresent)
            }
        }

        let wrapped_transposer = match &self.inner {
            SubStepInner::SaturatedInit {
                wrapped_transposer,
                ..
            } => wrapped_transposer,
            SubStepInner::SaturatedInput {
                wrapped_transposer,
                ..
            } => wrapped_transposer,
            SubStepInner::SaturatedScheduled {
                wrapped_transposer,
                ..
            } => wrapped_transposer,
            _ => return Err(NextUnsaturatedErr::NotSaturated),
        };
        let next_scheduled_time = wrapped_transposer.get_next_scheduled_time();

        let next_time_index = self.time.index() + 1;

        let (time, inner) = match (next_inputs.as_ref().map(|x| x.time()), next_scheduled_time) {
            (None, None) => return Ok(None),
            (None, Some(next_scheduled_time)) => (
                SubStepTime::new_scheduled(next_time_index, next_scheduled_time.clone()),
                SubStepInner::OriginalUnsaturatedScheduled {
                    input_state,
                },
            ),
            (Some(t_i), None) => (
                SubStepTime::new_input(next_time_index, t_i),
                SubStepInner::OriginalUnsaturatedInput {
                    inputs: core::mem::take(next_inputs).unwrap(),
                    input_state,
                },
            ),
            (Some(t_i), Some(t_s)) => match t_i.cmp(&t_s.time) {
                Ordering::Greater => (
                    SubStepTime::new_scheduled(next_time_index, t_s.clone()),
                    SubStepInner::OriginalUnsaturatedScheduled {
                        input_state,
                    },
                ),
                _ => (
                    SubStepTime::new_input(next_time_index, t_i),
                    SubStepInner::OriginalUnsaturatedInput {
                        inputs: core::mem::take(next_inputs).unwrap(),
                        input_state,
                    },
                ),
            },
        };

        let item = SubStep {
            time,
            inner,

            #[cfg(debug_assertions)]
            uuid_self: uuid::Uuid::new_v4(),
            #[cfg(debug_assertions)]
            uuid_prev: Some(self.uuid_self),
        };

        Ok(Some(item))
    }

    pub fn next_scheduled_unsaturated(
        &self,
        input_state: S::LazyState<Is>,
    ) -> Result<Option<Self>, NextUnsaturatedErr> {
        let wrapped_transposer = match &self.inner {
            SubStepInner::SaturatedInit {
                wrapped_transposer,
                ..
            } => wrapped_transposer,
            SubStepInner::SaturatedInput {
                wrapped_transposer,
                ..
            } => wrapped_transposer,
            SubStepInner::SaturatedScheduled {
                wrapped_transposer,
                ..
            } => wrapped_transposer,
            _ => return Err(NextUnsaturatedErr::NotSaturated),
        };

        let next_scheduled_time = wrapped_transposer.get_next_scheduled_time();
        let next_scheduled_time = match next_scheduled_time {
            Some(t) => t,
            None => return Ok(None),
        };

        let next_time_index = self.time.index() + 1;
        let time = SubStepTime::new_scheduled(next_time_index, next_scheduled_time.clone());
        let inner = SubStepInner::OriginalUnsaturatedScheduled {
            input_state,
        };

        let item = SubStep {
            time,
            inner,

            #[cfg(debug_assertions)]
            uuid_self: uuid::Uuid::new_v4(),
            #[cfg(debug_assertions)]
            uuid_prev: Some(self.uuid_self),
        };

        Ok(Some(item))
    }

    pub fn next_unsaturated_same_time(
        &self,
        input_state: S::LazyState<Is>,
    ) -> Result<Result<Self, S::Transposer<WrappedTransposer<T, S>>>, NextUnsaturatedErr> {
        let wrapped_transposer = match &self.inner {
            SubStepInner::SaturatedInit {
                wrapped_transposer,
                ..
            } => return Ok(Err(wrapped_transposer.clone())),
            SubStepInner::SaturatedInput {
                wrapped_transposer,
                ..
            } => wrapped_transposer,
            SubStepInner::SaturatedScheduled {
                wrapped_transposer,
                ..
            } => wrapped_transposer,
            _ => return Err(NextUnsaturatedErr::NotSaturated),
        };

        let next_scheduled_time = wrapped_transposer.get_next_scheduled_time();
        let next_scheduled_time = match next_scheduled_time {
            Some(t) => t,
            None => return Ok(Err(wrapped_transposer.clone())),
        };

        if self.time().raw_time() != next_scheduled_time.time {
            return Ok(Err(wrapped_transposer.clone()))
        }

        let next_time_index = self.time.index() + 1;
        let time = SubStepTime::new_scheduled(next_time_index, next_scheduled_time.clone());
        let inner = SubStepInner::OriginalUnsaturatedScheduled {
            input_state,
        };

        let item = SubStep {
            time,
            inner,

            #[cfg(debug_assertions)]
            uuid_self: uuid::Uuid::new_v4(),
            #[cfg(debug_assertions)]
            uuid_prev: Some(self.uuid_self),
        };

        Ok(Ok(item))
    }

    // previous is expected to be the value produced this via next_unsaturated.
    pub fn saturate_take(
        &mut self,
        previous_sub_step: &mut Self,
        previous_input_state: S::LazyState<Is>,
    ) -> Result<(), SaturateErr> {
        #[cfg(debug_assertions)]
        if self.uuid_prev != Some(previous_sub_step.uuid_self) {
            return Err(SaturateErr::IncorrectPrevious)
        }

        if !previous_sub_step.inner.is_saturated() {
            return Err(SaturateErr::PreviousNotSaturated)
        }

        if !self.inner.is_unsaturated() {
            return Err(SaturateErr::SelfNotUnsaturated)
        }

        let wrapped_transposer = replace_mut::replace_and_return(
            &mut previous_sub_step.inner,
            SubStepInner::recover,
            |prev| match prev {
                SubStepInner::SaturatedInit {
                    wrapped_transposer,
                } => (SubStepInner::UnsaturatedInit, wrapped_transposer),
                SubStepInner::SaturatedInput {
                    inputs,
                    wrapped_transposer,
                } => (
                    SubStepInner::RepeatUnsaturatedInput {
                        inputs,
                        input_state: previous_input_state,
                    },
                    wrapped_transposer,
                ),
                SubStepInner::SaturatedScheduled {
                    wrapped_transposer,
                } => (
                    SubStepInner::RepeatUnsaturatedScheduled {
                        input_state: previous_input_state,
                    },
                    wrapped_transposer,
                ),
                _ => unreachable!(),
            },
        );

        self.saturate_from_wrapped_transposer(wrapped_transposer);

        Ok(())
    }

    // previous is expected to be the value produced this via next_unsaturated.
    pub fn saturate_clone(&mut self, previous: &Self) -> Result<(), SaturateErr>
    where
        S::Transposer<WrappedTransposer<T, S>>: Clone,
    {
        #[cfg(debug_assertions)]
        if self.uuid_prev != Some(previous.uuid_self) {
            return Err(SaturateErr::IncorrectPrevious)
        }

        if !self.inner.is_unsaturated() {
            return Err(SaturateErr::SelfNotUnsaturated)
        }

        let wrapped_transposer: S::Transposer<WrappedTransposer<T, S>> = match &previous.inner {
            SubStepInner::SaturatedInit {
                wrapped_transposer,
                ..
            } => Ok(wrapped_transposer.clone()),
            SubStepInner::SaturatedInput {
                wrapped_transposer,
                ..
            } => Ok(wrapped_transposer.clone()),
            SubStepInner::SaturatedScheduled {
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
        wrapped_transposer: S::Transposer<WrappedTransposer<T, S>>,
    ) {
        replace_mut::replace(&mut self.inner, SubStepInner::recover, |next| match next {
            SubStepInner::OriginalUnsaturatedInput {
                inputs,
                input_state,
            } => {
                let update = Update::new::<SubStepUpdateContextFamily<T, S>>(
                    wrapped_transposer,
                    InputArg::new(inputs),
                    self.time.clone(),
                    input_state,
                );
                SubStepInner::OriginalSaturatingInput {
                    update,
                }
            },
            SubStepInner::RepeatUnsaturatedInput {
                inputs,
                input_state,
            } => {
                let update = Update::new::<SubStepUpdateContextFamily<T, S>>(
                    wrapped_transposer,
                    InputArg::new(inputs),
                    self.time.clone(),
                    input_state,
                );
                SubStepInner::RepeatSaturatingInput {
                    update,
                }
            },
            SubStepInner::OriginalUnsaturatedScheduled {
                input_state,
            } => {
                let update = Update::new::<SubStepUpdateContextFamily<T, S>>(
                    wrapped_transposer,
                    ScheduledArg::new(),
                    self.time.clone(),
                    input_state,
                );
                SubStepInner::OriginalSaturatingScheduled {
                    update,
                }
            },
            SubStepInner::RepeatUnsaturatedScheduled {
                input_state,
            } => {
                let update = Update::new::<SubStepUpdateContextFamily<T, S>>(
                    wrapped_transposer,
                    ScheduledArg::new(),
                    self.time.clone(),
                    input_state,
                );
                SubStepInner::RepeatSaturatingScheduled {
                    update,
                }
            },
            _ => unreachable!(),
        });
    }

    pub fn desaturate(&mut self, input_state: S::LazyState<Is>) -> Result<(), DesaturateErr> {
        replace_mut::replace_and_return(&mut self.inner, SubStepInner::recover, |original| {
            match original {
                SubStepInner::OriginalSaturatingInput {
                    update,
                } => {
                    let inner = SubStepInner::OriginalUnsaturatedInput {
                        inputs: update.reclaim_args().into_inputs(),
                        input_state,
                    };
                    (inner, Ok(()))
                },
                SubStepInner::RepeatSaturatingInput {
                    update,
                } => {
                    let inner = SubStepInner::RepeatUnsaturatedInput {
                        inputs: update.reclaim_args().into_inputs(),
                        input_state,
                    };
                    (inner, Ok(()))
                },
                SubStepInner::OriginalSaturatingScheduled {
                    update: _,
                } => (
                    SubStepInner::OriginalUnsaturatedScheduled {
                        input_state,
                    },
                    Ok(()),
                ),
                SubStepInner::RepeatSaturatingScheduled {
                    update: _,
                } => (
                    SubStepInner::RepeatUnsaturatedScheduled {
                        input_state,
                    },
                    Ok(()),
                ),
                SubStepInner::SaturatedInit {
                    wrapped_transposer: _,
                } => (SubStepInner::UnsaturatedInit, Ok(())),
                SubStepInner::SaturatedInput {
                    inputs,
                    wrapped_transposer: _,
                } => (
                    SubStepInner::RepeatUnsaturatedInput {
                        inputs,
                        input_state,
                    },
                    Ok(()),
                ),
                SubStepInner::SaturatedScheduled {
                    wrapped_transposer: _,
                } => (
                    SubStepInner::RepeatUnsaturatedScheduled {
                        input_state,
                    },
                    Ok(()),
                ),
                other => (other, Err(DesaturateErr::AlreadyUnsaturated)),
            }
        })
    }

    pub fn time(&self) -> &SubStepTime<T::Time> {
        &self.time
    }

    // this has the additional gurantee that the pointer returned lives until this is desaturated or dropped.
    // if you move self the returned pointer is still valid.
    pub fn finished_wrapped_transposer(&self) -> Option<&S::Transposer<WrappedTransposer<T, S>>> {
        match &self.inner {
            SubStepInner::SaturatedInit {
                wrapped_transposer,
            } => Some(wrapped_transposer),
            SubStepInner::SaturatedInput {
                wrapped_transposer,
                ..
            } => Some(wrapped_transposer),
            SubStepInner::SaturatedScheduled {
                wrapped_transposer,
            } => Some(wrapped_transposer),
            _ => None,
        }
    }

    pub fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Result<SubStepPoll<T, Is, S>, PollErr> {
        replace_mut::replace_and_return(&mut self.inner, SubStepInner::recover, |inner| match inner
        {
            SubStepInner::SaturatingInit {
                mut update,
            } => match Pin::new(&mut update).poll(cx) {
                UpdatePoll::Pending => (
                    SubStepInner::SaturatingInit {
                        update,
                    },
                    Ok(SubStepPoll::Pending),
                ),
                UpdatePoll::Event(output) => (
                    SubStepInner::SaturatingInit {
                        update,
                    },
                    Ok(SubStepPoll::Emitted(output)),
                ),
                UpdatePoll::Ready(wrapped_transposer, input_state) => (
                    SubStepInner::SaturatedInit {
                        wrapped_transposer,
                    },
                    Ok(SubStepPoll::Ready(input_state)),
                ),
            },
            SubStepInner::OriginalSaturatingInput {
                mut update,
            } => match Pin::new(&mut update).poll(cx) {
                UpdatePoll::Pending => (
                    SubStepInner::OriginalSaturatingInput {
                        update,
                    },
                    Ok(SubStepPoll::Pending),
                ),
                UpdatePoll::Event(output) => (
                    SubStepInner::OriginalSaturatingInput {
                        update,
                    },
                    Ok(SubStepPoll::Emitted(output)),
                ),
                UpdatePoll::Ready(wrapped_transposer, input_state) => {
                    let inputs = update.reclaim_args().into_inputs();
                    (
                        SubStepInner::SaturatedInput {
                            wrapped_transposer,
                            inputs,
                        },
                        Ok(SubStepPoll::Ready(input_state)),
                    )
                },
            },
            SubStepInner::RepeatSaturatingInput {
                mut update,
            } => match Pin::new(&mut update).poll(cx) {
                UpdatePoll::Pending => (
                    SubStepInner::RepeatSaturatingInput {
                        update,
                    },
                    Ok(SubStepPoll::Pending),
                ),
                UpdatePoll::Event(_) => unreachable!(),
                UpdatePoll::Ready(wrapped_transposer, input_state) => {
                    let inputs = update.reclaim_args().into_inputs();
                    (
                        SubStepInner::SaturatedInput {
                            wrapped_transposer,
                            inputs,
                        },
                        Ok(SubStepPoll::Ready(input_state)),
                    )
                },
            },
            SubStepInner::OriginalSaturatingScheduled {
                mut update,
            } => match Pin::new(&mut update).poll(cx) {
                UpdatePoll::Pending => (
                    SubStepInner::OriginalSaturatingScheduled {
                        update,
                    },
                    Ok(SubStepPoll::Pending),
                ),
                UpdatePoll::Event(output) => (
                    SubStepInner::OriginalSaturatingScheduled {
                        update,
                    },
                    Ok(SubStepPoll::Emitted(output)),
                ),
                UpdatePoll::Ready(wrapped_transposer, input_state) => (
                    SubStepInner::SaturatedScheduled {
                        wrapped_transposer,
                    },
                    Ok(SubStepPoll::Ready(input_state)),
                ),
            },
            SubStepInner::RepeatSaturatingScheduled {
                mut update,
            } => match Pin::new(&mut update).poll(cx) {
                UpdatePoll::Pending => (
                    SubStepInner::RepeatSaturatingScheduled {
                        update,
                    },
                    Ok(SubStepPoll::Pending),
                ),
                UpdatePoll::Event(_) => unreachable!(),
                UpdatePoll::Ready(wrapped_transposer, input_state) => (
                    SubStepInner::SaturatedScheduled {
                        wrapped_transposer,
                    },
                    Ok(SubStepPoll::Ready(input_state)),
                ),
            },
            SubStepInner::UnsaturatedInit
            | SubStepInner::OriginalUnsaturatedInput {
                ..
            }
            | SubStepInner::OriginalUnsaturatedScheduled {
                ..
            }
            | SubStepInner::RepeatUnsaturatedInput {
                ..
            }
            | SubStepInner::RepeatUnsaturatedScheduled {
                ..
            } => (inner, Err(PollErr::Unsaturated)),
            SubStepInner::SaturatedInit {
                ..
            }
            | SubStepInner::SaturatedInput {
                ..
            }
            | SubStepInner::SaturatedScheduled {
                ..
            } => (inner, Err(PollErr::Saturated)),
        })
    }
}

enum SubStepInner<'almost_static, T: Transposer, S: StorageFamily, Is: InputState<T>>
where
    (T, Is): 'almost_static,
{
    // notably this can never be rehydrated because you need the preceding wrapped_transposer
    // and there isn't one, because this is init.
    UnsaturatedInit,
    OriginalUnsaturatedInput {
        inputs:      StepInputs<T>,
        input_state: S::LazyState<Is>,
    },
    OriginalUnsaturatedScheduled {
        input_state: S::LazyState<Is>,
    },
    RepeatUnsaturatedInput {
        inputs:      StepInputs<T>,
        input_state: S::LazyState<Is>,
    },
    RepeatUnsaturatedScheduled {
        input_state: S::LazyState<Is>,
    },
    SaturatingInit {
        update: Update<'almost_static, T, S, InitArg<T>, Is>,
    },
    OriginalSaturatingInput {
        update: Update<'almost_static, T, S, InputArg<T>, Is>,
    },
    OriginalSaturatingScheduled {
        update: Update<'almost_static, T, S, ScheduledArg<T>, Is>,
    },
    RepeatSaturatingInput {
        update: Update<'almost_static, T, S, InputArg<T>, Is>,
    },
    RepeatSaturatingScheduled {
        update: Update<'almost_static, T, S, ScheduledArg<T>, Is>,
    },
    SaturatedInit {
        wrapped_transposer: S::Transposer<WrappedTransposer<T, S>>,
    },
    SaturatedInput {
        inputs:             StepInputs<T>,
        wrapped_transposer: S::Transposer<WrappedTransposer<T, S>>,
    },
    SaturatedScheduled {
        wrapped_transposer: S::Transposer<WrappedTransposer<T, S>>,
    },
}

impl<'almost_static, T: Transposer, S: StorageFamily, Is: InputState<T>>
    SubStepInner<'almost_static, T, S, Is>
{
    fn recover() -> Self {
        Self::UnsaturatedInit
    }

    pub fn is_unsaturated(&self) -> bool {
        matches!(
            self,
            SubStepInner::OriginalUnsaturatedInput { .. }
                | SubStepInner::OriginalUnsaturatedScheduled { .. }
                | SubStepInner::RepeatUnsaturatedInput { .. }
                | SubStepInner::RepeatUnsaturatedScheduled { .. }
        )
    }

    pub fn is_saturated(&self) -> bool {
        matches!(
            self,
            SubStepInner::SaturatedInit { .. }
                | SubStepInner::SaturatedInput { .. }
                | SubStepInner::SaturatedScheduled { .. }
        )
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SubStepPoll<T: Transposer, Is: InputState<T>, S: StorageFamily> {
    Emitted(T::OutputEvent),
    Pending,
    Ready(S::LazyState<Is>),
}
