use core::pin::Pin;
use core::task::{Context, Waker};

use util::replace_mut;

use super::interpolation::Interpolation;
use super::step_inputs::StepInputs;
use super::sub_step::{PollErr as StepPollErr, SubStep, SubStepPoll, WrappedTransposer};
// use super::interpolation::Interpolation;
// use super::lazy_state::LazyState;
// use super::step_metadata::{EmptyStepMetadata, StepMetadata};
// use super::sub_step::{PollErr as StepPollErr, SubStep, SubStepTime, WrappedTransposer};
use crate::schedule_storage::{DefaultStorage, RefCounted, StorageFamily};
use crate::step::sub_step::SaturateErr;
// use crate::step::sub_step::SaturateErr;
use crate::{Transposer, TransposerInput};

pub struct Step<'almost_static, T: Transposer, Is: InputState<T>, S: StorageFamily = DefaultStorage>
where
    (T, Is): 'almost_static,
{
    inner:       StepInner<'almost_static, T, Is, S>,
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

impl<'almost_static, T: Transposer, Is: InputState<T>, S: StorageFamily>
    Step<'almost_static, T, Is, S>
{
    pub fn new_init(transposer: T, rng_seed: [u8; 32]) -> Self {
        let input_state = S::LazyState::new(Box::new(Is::new()));

        let steps = vec![SubStep::new_init(transposer, rng_seed, input_state.clone())];

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
        let input_state = S::LazyState::new(Box::new(Is::new()));
        if let StepInner::Saturated {
            steps, ..
        } = &self.inner
        {
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

    pub fn next_scheduled_unsaturated(&self) -> Result<Option<Self>, NextUnsaturatedErr> {
        let input_state = S::LazyState::new(Box::new(Is::new()));
        let steps = match &self.inner {
            StepInner::Saturated {
                steps, ..
            } => steps,
            _ => return Err(NextUnsaturatedErr::NotSaturated),
        };

        let next = steps
            .last()
            .unwrap()
            .next_scheduled_unsaturated(input_state.clone())
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
            prev_input_state: &S::LazyState<Is>,
            next: &mut SubStep<'a, T, Is, S>,
        ) -> Result<(), SaturateErr> {
            replace_mut::replace_and_return(prev, StepInner::recover, |inner| {
                if let StepInner::Saturated {
                    mut steps,
                    finished_wrapped_transposer,
                } = inner
                {
                    let last_step = steps.last_mut().unwrap();
                    match next.saturate_take(last_step, prev_input_state.clone()) {
                        Err(err) => {
                            let replacement = StepInner::Saturated {
                                steps,
                                finished_wrapped_transposer,
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

        self.saturate(|next| take_from_previous(&mut prev.inner, &prev.input_state, next))
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
        let input_state = S::LazyState::new(Box::new(Is::new()));
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
                    .desaturate(input_state)
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
                    .desaturate(input_state)
                    .unwrap();
                (
                    StepInner::RepeatUnsaturated {
                        steps,
                    },
                    Ok(()),
                )
            },
            StepInner::Saturated {
                steps, ..
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

            let input_state = match poll_result {
                SubStepPoll::Ready(i) => i,
                SubStepPoll::Emitted(e) => return Ok(StepPoll::Emitted(e)),
                SubStepPoll::Pending => return Ok(StepPoll::Pending),
            };

            // now we are ready, we need to advance to the next sub-step.
            if self.advance_saturation_index(input_state).is_saturated() {
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

    fn advance_saturation_index(
        &mut self,
        input_state: S::LazyState<Is>,
    ) -> AdvanceSaturationIndex {
        let mut x = None;
        let (i, steps) = match &mut self.inner {
            StepInner::OriginalSaturating {
                current_saturating_index: i,
                steps,
                ..
            } => {
                if *i == steps.len() - 1 {
                    let next = steps
                        .last()
                        .unwrap()
                        .next_unsaturated_same_time(input_state)
                        .unwrap();
                    match next {
                        Ok(next) => steps.push(next),
                        Err(wrapped_transposer) => x = Some(wrapped_transposer),
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
                    finished_wrapped_transposer: x.unwrap(),
                }
            });
            return AdvanceSaturationIndex::Saturated
        }

        *i += 1;
        let (part1, part2) = steps.split_at_mut(*i);
        let prev = part1.last_mut().unwrap();
        let next = part2.first_mut().unwrap();
        next.saturate_take(prev, self.input_state.clone()).unwrap();

        AdvanceSaturationIndex::Saturating
    }

    pub fn interpolate(
        &self,
        time: T::Time,
    ) -> Result<Interpolation<'almost_static, T, Is, S>, InterpolateErr> {
        let wrapped_transposer = match &self.inner {
            StepInner::Saturated {
                finished_wrapped_transposer,
                ..
            } => finished_wrapped_transposer.clone(),
            _ => return Err(InterpolateErr::NotSaturated),
        };

        let base_time = self.get_time();

        if time < base_time {
            return Err(InterpolateErr::TimePast)
        }

        Ok(Interpolation::new(base_time, time, wrapped_transposer))
    }

    pub fn get_input_state(&self) -> &Is {
        &self.input_state
    }

    pub fn get_time(&self) -> T::Time {
        self.get_steps().first().unwrap().time().raw_time()
    }

    fn get_steps(&self) -> &[SubStep<'almost_static, T, Is, S>] {
        match &self.inner {
            StepInner::OriginalUnsaturated {
                steps, ..
            } => steps,
            StepInner::OriginalSaturating {
                steps, ..
            } => steps,
            StepInner::RepeatUnsaturated {
                steps, ..
            } => steps,
            StepInner::RepeatSaturating {
                steps, ..
            } => steps,
            StepInner::Saturated {
                steps, ..
            } => steps,
        }
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
        finished_wrapped_transposer: S::Transposer<WrappedTransposer<T, S>>,
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
