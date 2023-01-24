use core::pin::Pin;
use core::task::{Context, Waker};
use std::collections::{BTreeMap, BTreeSet};
use std::marker::PhantomData;
use std::sync::Arc;

use futures_core::Future;
use util::replace_mut;

use super::step_inputs::StepInputs;
use super::sub_step::{PollErr as StepPollErr, SubStep, SubStepTime, WrappedTransposer};
use crate::context::HandleInputContext;
// use super::interpolation::Interpolation;
// use super::lazy_state::LazyState;
// use super::step_metadata::{EmptyStepMetadata, StepMetadata};
// use super::sub_step::{PollErr as StepPollErr, SubStep, SubStepTime, WrappedTransposer};
use crate::schedule_storage::{DefaultStorage, RefCounted, StorageFamily};
use crate::step::sub_step::SaturateErr;
// use crate::step::sub_step::SaturateErr;
use crate::{Transposer, TransposerInput, TransposerInputEventHandler};

pub struct Step<'almost_static, T: Transposer, Is: InputState<T>, S: StorageFamily = DefaultStorage>
where
    (T, Is): 'almost_static,
{
    inner: StepInner<'almost_static, T, Is, S>,

    input_state: S::LazyState<Is>,

    // these are used purely for enforcing that saturate calls use the previous step_group.
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

impl<'almost_static, T: Transposer, Is: InputState<T>, S: StorageFamily>
    Step<'almost_static, T, Is, S>
{
    pub fn new_init(transposer: T, rng_seed: [u8; 32]) -> Self {
        let input_state = S::LazyState::new(Box::new(Is::new()));

        let mut steps = Vec::with_capacity(1);
        steps.push(SubStep::new_init(transposer, rng_seed, input_state.clone()));

        Self {
            inner: StepInner::OriginalSaturating {
                current_saturating_index: 0,
                steps,
            },
            input_state,

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
        if let StepInner::Saturated {
            steps,
        } = &self.inner
        {
            let input_state = S::LazyState::new(Box::new(Is::new()));

            let next = steps
                .last()
                .unwrap()
                .next_unsaturated(next_inputs, input_state.clone())
                .map_err(|e| match e {
                    super::sub_step::NextUnsaturatedErr::NotSaturated => unreachable!(),

                    #[cfg(debug_assertions)]
                    super::sub_step::NextUnsaturatedErr::InputPastOrPresent => {
                        NextUnsaturatedErr::InputPastOrPresent
                    },
                })?;

            let next = next.map(|step| Step {
                inner: StepInner::OriginalUnsaturated {
                    steps: vec![step]
                },
                input_state,

                #[cfg(debug_assertions)]
                uuid_self: uuid::Uuid::new_v4(),
                #[cfg(debug_assertions)]
                uuid_prev: Some(self.uuid_self),
            });

            Ok(next)
        } else {
            Err(NextUnsaturatedErr::NotSaturated)
        }
    }

    fn saturate<F, E>(&mut self, saturate_first_step: F) -> Result<(), Option<E>>
    where
        F: FnOnce(&mut SubStep<'almost_static, T, Is, S>) -> Result<(), E>,
    {
        replace_mut::replace_and_return(&mut self.inner, StepInner::recover, |inner| match inner {
            StepInner::OriginalUnsaturated {
                mut steps,
            } => match saturate_first_step(steps.first_mut().unwrap()) {
                Ok(()) => {
                    let replacement = StepInner::OriginalSaturating {
                        current_saturating_index: 0,
                        steps,
                    };
                    (replacement, Ok(()))
                },
                Err(e) => (
                    StepInner::OriginalUnsaturated {
                        steps,
                    },
                    Err(Some(e)),
                ),
            },
            StepInner::RepeatUnsaturated {
                mut steps,
            } => match saturate_first_step(steps.first_mut().unwrap()) {
                Ok(()) => {
                    let replacement = StepInner::RepeatSaturating {
                        current_saturating_index: 0,
                        steps,
                    };
                    (replacement, Ok(()))
                },
                Err(e) => (
                    StepInner::RepeatUnsaturated {
                        steps,
                    },
                    Err(Some(e)),
                ),
            },
            _ => (inner, Err(None)),
        })
    }

    pub fn saturate_take(&mut self, prev: &mut Self) -> Result<(), SaturateTakeErr> {
        #[cfg(debug_assertions)]
        if self.uuid_prev != Some(prev.uuid_self) {
            return Err(SaturateTakeErr::IncorrectPrevious)
        }

        fn take_from_previous<'a, T: Transposer, Is: InputState<T>, S: StorageFamily>(
            prev: &mut StepInner<'a, T, Is, S>,
            next: &mut SubStep<'a, T, Is, S>,
        ) -> Result<(), SaturateErr> {
            replace_mut::replace_and_return(prev, StepInner::recover, |inner| {
                if let StepInner::Saturated {
                    mut steps,
                } = inner
                {
                    let last_step = steps.last_mut().unwrap();
                    match next.saturate_take(last_step) {
                        Err(err) => {
                            let replacement = StepInner::Saturated {
                                steps,
                            };
                            (replacement, Err(err))
                        },
                        Ok(()) => {
                            let replacement = StepInner::RepeatUnsaturated {
                                steps,
                            };

                            (replacement, Ok(()))
                        },
                    }
                } else {
                    (inner, Err(SaturateErr::PreviousNotSaturated))
                }
            })
        }

        self.saturate(|next| take_from_previous(&mut prev.inner, next))
            .map_err(|err| match err {
                Some(e) => match e {
                    SaturateErr::PreviousNotSaturated => SaturateTakeErr::PreviousNotSaturated,
                    SaturateErr::SelfNotUnsaturated => SaturateTakeErr::SelfNotUnsaturated,

                    #[cfg(debug_assertions)]
                    SaturateErr::IncorrectPrevious => SaturateTakeErr::IncorrectPrevious,
                },
                None => SaturateTakeErr::SelfNotUnsaturated,
            })
    }

    pub fn saturate_clone(&mut self, prev: &Self) -> Result<(), SaturateCloneErr>
    where
        T: Clone,
        S::Transposer<WrappedTransposer<T, S>>: Clone,
    {
        #[cfg(debug_assertions)]
        if self.uuid_prev != Some(prev.uuid_self) {
            return Err(SaturateCloneErr::IncorrectPrevious)
        }

        fn clone_from_previous<'a, T: Transposer, Is: InputState<T>, S: StorageFamily>(
            prev: &StepInner<'a, T, Is, S>,
            next: &mut SubStep<'a, T, Is, S>,
        ) -> Result<(), SaturateErr>
        where
            S::Transposer<WrappedTransposer<T, S>>: Clone,
        {
            if let StepInner::Saturated {
                steps, ..
            } = prev
            {
                next.saturate_clone(steps.last().unwrap())
            } else {
                Err(SaturateErr::PreviousNotSaturated)
            }
        }

        self.saturate(|next| clone_from_previous(&prev.inner, next))
            .map_err(|err| match err {
                Some(SaturateErr::PreviousNotSaturated) => SaturateCloneErr::PreviousNotSaturated,
                Some(SaturateErr::SelfNotUnsaturated) => SaturateCloneErr::SelfNotUnsaturated,

                #[cfg(debug_assertions)]
                Some(SaturateErr::IncorrectPrevious) => SaturateCloneErr::IncorrectPrevious,
                None => SaturateCloneErr::SelfNotUnsaturated,
            })
    }

    pub fn desaturate(&mut self) -> Result<(), DesaturateErr> {
        replace_mut::replace_and_return(&mut self.inner, StepInner::recover, |inner| match inner {
            StepInner::OriginalUnsaturated {
                ..
            }
            | StepInner::RepeatUnsaturated {
                ..
            } => (inner, Err(DesaturateErr::AlreadyUnsaturated)),
            StepInner::OriginalSaturating {
                current_saturating_index,
                mut steps,
            } => {
                steps
                    .get_mut(current_saturating_index)
                    .unwrap()
                    .desaturate()
                    .unwrap();
                (
                    StepInner::OriginalUnsaturated {
                        steps,
                    },
                    Ok(()),
                )
            },
            StepInner::RepeatSaturating {
                current_saturating_index,
                mut steps,
            } => {
                steps
                    .get_mut(current_saturating_index)
                    .unwrap()
                    .desaturate()
                    .unwrap();
                (
                    StepInner::RepeatUnsaturated {
                        steps,
                    },
                    Ok(()),
                )
            },
            StepInner::Saturated {
                steps,
            } => (
                StepInner::RepeatUnsaturated {
                    steps,
                },
                Ok(()),
            ),
        })
    }

    pub fn poll(&mut self, waker: Waker) -> Result<StepPoll<T>, PollErr> {
        loop {
            let CurrentSaturating {
                sub_step,
            } = match self.current_saturating() {
                Ok(x) => x,
                Err(CurrentSaturatingErr::Unsaturated) => return Err(PollErr::Unsaturated),
                Err(CurrentSaturatingErr::Saturated) => unreachable!(),
            };
            let sub_step = Pin::new(sub_step);
            let mut cx = Context::from_waker(&waker);
            let poll_result = sub_step.poll(&mut cx).map_err(|e| match e {
                StepPollErr::Unsaturated => PollErr::Unsaturated,
                StepPollErr::Saturated => PollErr::Saturated,
            })?;

            match poll_result {
                StepPoll::Ready => {},
                r => return Ok(r),
            };

            // now we are ready, we need to advance to the next sub-step.
            if self.advance_saturation_index().is_saturated() {
                break Ok(StepPoll::Ready)
            }
        }
    }

    fn current_saturating(
        &mut self,
    ) -> Result<CurrentSaturating<'_, 'almost_static, T, Is, S>, CurrentSaturatingErr> {
        match &mut self.inner {
            StepInner::OriginalSaturating {
                current_saturating_index,
                steps,
            } => Ok(CurrentSaturating {
                sub_step: steps.get_mut(*current_saturating_index).unwrap(),
            }),
            StepInner::RepeatSaturating {
                current_saturating_index,
                steps,
            } => Ok(CurrentSaturating {
                sub_step: steps.get_mut(*current_saturating_index).unwrap(),
            }),
            StepInner::OriginalUnsaturated {
                ..
            } => Err(CurrentSaturatingErr::Unsaturated),
            StepInner::RepeatUnsaturated {
                ..
            } => Err(CurrentSaturatingErr::Unsaturated),
            StepInner::Saturated {
                ..
            } => Err(CurrentSaturatingErr::Saturated),
        }
    }

    fn advance_saturation_index(&mut self) -> AdvanceSaturationIndex {
        let (i, steps) = match &mut self.inner {
            StepInner::OriginalSaturating {
                current_saturating_index: i,
                steps,
                ..
            } => {
                if *i == steps.len() - 1 {
                    let next = steps.last().unwrap().next_unsaturated_same_time().unwrap();
                    if let Some(next) = next {
                        steps.push(next);
                    }
                }

                (i, steps.as_mut_slice())
            },
            StepInner::RepeatSaturating {
                current_saturating_index: i,
                steps,
                ..
            } => (i, steps.as_mut()),
            _ => unreachable!(),
        };

        if *i == steps.len() - 1 {
            // convert self to saturated
            replace_mut::replace(&mut self.inner, StepInner::recover, |inner| {
                let steps = match inner {
                    StepInner::OriginalSaturating {
                        steps, ..
                    } => steps.into_boxed_slice(),
                    StepInner::RepeatSaturating {
                        steps, ..
                    } => steps,
                    _ => unreachable!(),
                };

                StepInner::Saturated {
                    steps,
                }
            });
            return AdvanceSaturationIndex::Saturated
        }

        *i += 1;
        let (part1, part2) = steps.split_at_mut(*i);
        let prev = part1.last_mut().unwrap();
        let next = part2.first_mut().unwrap();
        next.saturate_take(prev).unwrap();

        AdvanceSaturationIndex::Saturating
    }

    // pub fn interpolate(&self, time: T::Time) -> Result<Interpolation<T, S>, InterpolateErr> {
    //     todo!()
    // }

    pub fn get_input_state(&self) -> &Is {
        &self.input_state
    }
}

enum StepInner<'almost_static, T: Transposer, Is: InputState<T>, S: StorageFamily> {
    OriginalUnsaturated {
        steps: Vec<SubStep<'almost_static, T, Is, S>>,
    },
    OriginalSaturating {
        current_saturating_index: usize,
        steps:                    Vec<SubStep<'almost_static, T, Is, S>>,
    },
    RepeatUnsaturated {
        steps: Box<[SubStep<'almost_static, T, Is, S>]>,
    },
    RepeatSaturating {
        current_saturating_index: usize,
        steps:                    Box<[SubStep<'almost_static, T, Is, S>]>,
    },
    Saturated {
        steps: Box<[SubStep<'almost_static, T, Is, S>]>,
    },
}

impl<'almost_static, T: Transposer, Is: InputState<T>, S: StorageFamily>
    StepInner<'almost_static, T, Is, S>
{
    fn recover() -> Self {
        todo!()
    }
}

enum AdvanceSaturationIndex {
    Saturating,
    Saturated,
}

impl AdvanceSaturationIndex {
    pub fn is_saturated(&self) -> bool {
        match self {
            Self::Saturating => false,
            Self::Saturated => true,
        }
    }
}

struct CurrentSaturating<'a, 'almost_static, T: Transposer, Is: InputState<T>, S: StorageFamily> {
    sub_step: &'a mut SubStep<'almost_static, T, Is, S>,
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
enum CurrentSaturatingErr {
    Unsaturated,
    Saturated,
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

#[derive(Debug)]
pub enum DesaturateErr {
    AlreadyUnsaturated,
    ActiveWakers,
    ActiveInterpolations,
}
