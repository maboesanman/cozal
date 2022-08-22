use core::pin::Pin;
use core::task::{Context, Waker};
use std::sync::Arc;

use util::replace_mut;

use super::interpolation::Interpolation;
use super::lazy_state::LazyState;
use super::step_metadata::{EmptyStepMetadata, StepMetadata};
use super::sub_step::{PollErr as StepPollErr, SubStep, SubStepTime, WrappedTransposer};
use crate::schedule_storage::{DefaultStorage, StorageFamily, TransposerPointer};
use crate::step::sub_step::SaturateErr;
use crate::Transposer;

pub struct Step<
    T: Transposer,
    S: StorageFamily = DefaultStorage,
    M: StepMetadata<T, S> = EmptyStepMetadata,
> {
    inner: StepInner<T, S, M>,

    input_state: LazyState<T::InputState>,

    // these are used purely for enforcing that saturate calls use the previous step_group.
    #[cfg(debug_assertions)]
    uuid_self: uuid::Uuid,
    #[cfg(debug_assertions)]
    uuid_prev: Option<uuid::Uuid>,
}

pub type NextInputs<T> = Option<(<T as Transposer>::Time, Box<[<T as Transposer>::Input]>)>;

impl<T: Transposer, S: StorageFamily, M: StepMetadata<T, S>> Step<T, S, M> {
    pub fn new_init(transposer: T, rng_seed: [u8; 32]) -> Self {
        let mut steps = Vec::with_capacity(1);
        let input_state = LazyState::new();

        steps.push(SubStep::new_init(transposer, rng_seed, &input_state));

        Self {
            inner: StepInner::OriginalSaturating {
                current_saturating_index: 0,
                steps,
                metadata: M::new_init(),
            },
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
    ) -> Result<Option<Self>, NextUnsaturatedErr> {
        if let StepInner::Saturated {
            steps,
            metadata,
        } = &self.inner
        {
            let input_state = LazyState::new();

            let next = steps
                .last()
                .unwrap()
                .next_unsaturated(next_inputs, &input_state)
                .map_err(|e| match e {
                    super::sub_step::NextUnsaturatedErr::NotSaturated => unreachable!(),

                    #[cfg(debug_assertions)]
                    super::sub_step::NextUnsaturatedErr::InputPastOrPresent => {
                        NextUnsaturatedErr::InputPastOrPresent
                    },
                })?;

            let next = next.map(|step| Step {
                inner: StepInner::OriginalUnsaturated {
                    steps:    vec![step],
                    metadata: M::next_unsaturated(metadata),
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

    pub fn saturate_take(&mut self, prev: &mut Self) -> Result<(), SaturateTakeErr> {
        #[cfg(debug_assertions)]
        if self.uuid_prev != Some(prev.uuid_self) {
            return Err(SaturateTakeErr::IncorrectPrevious)
        }

        fn take_from_previous<T: Transposer, S: StorageFamily, M: StepMetadata<T, S>>(
            prev: &mut StepInner<T, S, M>,
            next: &mut SubStep<T, S>,
        ) -> Result<(), SaturateErr> {
            replace_mut::replace_and_return(
                prev,
                || StepInner::Unreachable,
                |inner| {
                    if let StepInner::Saturated {
                        mut steps,
                        metadata,
                    } = inner
                    {
                        let last_step = steps.last_mut().unwrap();
                        let ptr = last_step.finished_wrapped_transposer().unwrap().borrow();
                        match next.saturate_take(last_step) {
                            Err(err) => {
                                let replacement = StepInner::Saturated {
                                    steps,
                                    metadata,
                                };
                                (replacement, Err(err))
                            },
                            Ok(()) => {
                                let replacement = StepInner::RepeatUnsaturated {
                                    steps,
                                    metadata: M::desaturate_saturated(metadata, &ptr.transposer),
                                };

                                (replacement, Ok(()))
                            },
                        }
                    } else {
                        (inner, Err(SaturateErr::PreviousNotSaturated))
                    }
                },
            )
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
        S::Transposer<WrappedTransposer<T, S>>: Clone,
    {
        #[cfg(debug_assertions)]
        if self.uuid_prev != Some(prev.uuid_self) {
            return Err(SaturateCloneErr::IncorrectPrevious)
        }

        fn clone_from_previous<T: Transposer, S: StorageFamily, M: StepMetadata<T, S>>(
            prev: &StepInner<T, S, M>,
            next: &mut SubStep<T, S>,
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

    fn saturate<F, E>(&mut self, saturate_first_step: F) -> Result<(), Option<E>>
    where
        F: FnOnce(&mut SubStep<T, S>) -> Result<(), E>,
    {
        replace_mut::replace_and_return(
            &mut self.inner,
            || StepInner::Unreachable,
            |inner| match inner {
                StepInner::OriginalUnsaturated {
                    mut steps,
                    metadata,
                } => match saturate_first_step(steps.first_mut().unwrap()) {
                    Ok(()) => {
                        let replacement = StepInner::OriginalSaturating {
                            current_saturating_index: 0,
                            steps,
                            metadata: M::original_saturating(metadata),
                        };
                        (replacement, Ok(()))
                    },
                    Err(e) => (
                        StepInner::OriginalUnsaturated {
                            steps,
                            metadata,
                        },
                        Err(Some(e)),
                    ),
                },
                StepInner::RepeatUnsaturated {
                    mut steps,
                    metadata,
                } => match saturate_first_step(steps.first_mut().unwrap()) {
                    Ok(()) => {
                        let replacement = StepInner::RepeatSaturating {
                            current_saturating_index: 0,
                            steps,
                            metadata: M::repeat_saturating(metadata),
                        };
                        (replacement, Ok(()))
                    },
                    Err(e) => (
                        StepInner::RepeatUnsaturated {
                            steps,
                            metadata,
                        },
                        Err(Some(e)),
                    ),
                },
                StepInner::Unreachable => unreachable!(),
                _ => (inner, Err(None)),
            },
        )
    }

    pub fn desaturate(&mut self) -> Result<(), DesaturateErr> {
        replace_mut::replace_and_return(
            &mut self.inner,
            || StepInner::Unreachable,
            |inner| match inner {
                StepInner::OriginalUnsaturated {
                    ..
                }
                | StepInner::RepeatUnsaturated {
                    ..
                } => (inner, Err(DesaturateErr::AlreadyUnsaturated)),
                StepInner::OriginalSaturating {
                    current_saturating_index,
                    mut steps,
                    metadata,
                } => {
                    steps
                        .get_mut(current_saturating_index)
                        .unwrap()
                        .desaturate()
                        .unwrap();
                    (
                        StepInner::OriginalUnsaturated {
                            steps,
                            metadata: M::desaturate_original_saturating(metadata),
                        },
                        Ok(()),
                    )
                },
                StepInner::RepeatSaturating {
                    current_saturating_index,
                    mut steps,
                    metadata,
                } => {
                    steps
                        .get_mut(current_saturating_index)
                        .unwrap()
                        .desaturate()
                        .unwrap();
                    (
                        StepInner::RepeatUnsaturated {
                            steps,
                            metadata: M::desaturate_repeat_saturating(metadata),
                        },
                        Ok(()),
                    )
                },
                StepInner::Saturated {
                    steps,
                    metadata,
                } => {
                    let last_step = steps.last().unwrap();
                    let ptr = last_step.finished_wrapped_transposer().unwrap().borrow();
                    (
                        StepInner::RepeatUnsaturated {
                            steps,
                            metadata: M::desaturate_saturated(metadata, &ptr.transposer),
                        },
                        Ok(()),
                    )
                },
                StepInner::Unreachable => unreachable!(),
            },
        )
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
                StepPoll::NeedsState => return Ok(StepPoll::NeedsState),
                StepPoll::Emitted(o) => return Ok(StepPoll::Emitted(o)),
                StepPoll::Pending => return Ok(StepPoll::Pending),
            };

            // now we are ready, we need to advance to the next sub-step.
            if self.advance_saturation_index().is_saturated() {
                break Ok(StepPoll::Ready)
            }
        }
    }

    pub fn set_input_state(
        &mut self,
        state: T::InputState,
    ) -> Result<(), Arc<T::InputState>> {
        self.input_state.set(state)
    }

    pub fn interpolate(&self, time: T::Time) -> Result<Interpolation<T, S>, InterpolateErr> {
        match &self.inner {
            StepInner::Saturated {
                steps,
                metadata: _,
            } => {
                #[cfg(debug_assertions)]
                if self.raw_time() > time {
                    return Err(InterpolateErr::TimePast)
                }

                Ok(Interpolation::new(
                    time,
                    steps
                        .as_ref()
                        .last()
                        .unwrap()
                        .finished_wrapped_transposer()
                        .unwrap(),
                ))
            },
            _ => Err(InterpolateErr::NotSaturated),
        }
    }

    pub fn get_metadata(&self) -> Metadata<'_, T, S, M> {
        match &self.inner {
            StepInner::OriginalUnsaturated {
                metadata, ..
            } => Metadata::OriginalUnsaturated(metadata),
            StepInner::OriginalSaturating {
                metadata, ..
            } => Metadata::OriginalSaturating(metadata),
            StepInner::RepeatUnsaturated {
                metadata, ..
            } => Metadata::RepeatUnsaturated(metadata),
            StepInner::RepeatSaturating {
                metadata, ..
            } => Metadata::RepeatSaturating(metadata),
            StepInner::Saturated {
                metadata, ..
            } => Metadata::Saturated(metadata),
            StepInner::Unreachable => unreachable!(),
        }
    }

    pub fn get_metadata_mut(&mut self) -> MetadataMut<'_, T, S, M> {
        match &mut self.inner {
            StepInner::OriginalUnsaturated {
                metadata, ..
            } => MetadataMut::OriginalUnsaturated(metadata),
            StepInner::OriginalSaturating {
                metadata, ..
            } => MetadataMut::OriginalSaturating(metadata),
            StepInner::RepeatUnsaturated {
                metadata, ..
            } => MetadataMut::RepeatUnsaturated(metadata),
            StepInner::RepeatSaturating {
                metadata, ..
            } => MetadataMut::RepeatSaturating(metadata),
            StepInner::Saturated {
                metadata, ..
            } => MetadataMut::Saturated(metadata),
            StepInner::Unreachable => unreachable!(),
        }
    }

    fn first_step_time(&self) -> &SubStepTime<T::Time> {
        match &self.inner {
            StepInner::OriginalUnsaturated {
                steps, ..
            } => steps.first().unwrap().time(),
            StepInner::RepeatUnsaturated {
                steps, ..
            } => steps.first().unwrap().time(),
            StepInner::OriginalSaturating {
                steps, ..
            } => steps.first().unwrap().time(),
            StepInner::RepeatSaturating {
                steps, ..
            } => steps.first().unwrap().time(),
            StepInner::Saturated {
                steps, ..
            } => steps.first().unwrap().time(),
            StepInner::Unreachable => unreachable!(),
        }
    }

    fn current_saturating(&mut self) -> Result<CurrentSaturating<T, S>, CurrentSaturatingErr> {
        match &mut self.inner {
            StepInner::OriginalSaturating {
                current_saturating_index,
                steps,
                metadata: _,
            } => Ok(CurrentSaturating {
                sub_step: steps.get_mut(*current_saturating_index).unwrap(),
            }),
            StepInner::RepeatSaturating {
                current_saturating_index,
                steps,
                metadata: _,
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
            StepInner::Unreachable => unreachable!(),
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
            replace_mut::replace(
                &mut self.inner,
                || StepInner::Unreachable,
                |inner| {
                    let (steps, metadata) = match inner {
                        StepInner::OriginalSaturating {
                            steps,
                            metadata,
                            ..
                        } => {
                            let wrapped_transposer_ref =
                                steps.last().unwrap().finished_wrapped_transposer().unwrap();
                            let metadata =
                                M::original_saturate(metadata, &wrapped_transposer_ref.transposer);

                            (steps.into_boxed_slice(), metadata)
                        },
                        StepInner::RepeatSaturating {
                            steps,
                            metadata,
                            ..
                        } => {
                            let wrapped_transposer_ref =
                                steps.last().unwrap().finished_wrapped_transposer().unwrap();

                            let metadata =
                                M::repeat_saturate(metadata, &wrapped_transposer_ref.transposer);

                            (steps, metadata)
                        },
                        _ => unreachable!(),
                    };

                    StepInner::Saturated {
                        steps,
                        metadata,
                    }
                },
            );
            return AdvanceSaturationIndex::Saturated
        }

        *i += 1;
        let (part1, part2) = steps.split_at_mut(*i);
        let prev = part1.last_mut().unwrap();
        let next = part2.first_mut().unwrap();
        next.saturate_take(prev).unwrap();

        AdvanceSaturationIndex::Saturating
    }

    pub fn is_saturated(&self) -> bool {
        matches!(self.inner, StepInner::Saturated { .. })
    }

    pub fn is_saturating(&self) -> bool {
        matches!(
            self.inner,
            StepInner::OriginalSaturating { .. } | StepInner::RepeatSaturating { .. }
        )
    }

    pub fn is_unsaturated(&self) -> bool {
        matches!(
            self.inner,
            StepInner::OriginalUnsaturated { .. } | StepInner::RepeatUnsaturated { .. }
        )
    }

    pub fn is_init(&self) -> bool {
        self.first_step_time().index() == 0
    }

    pub fn raw_time(&self) -> T::Time {
        self.first_step_time().raw_time()
    }

    pub fn time(&self) -> StepTime<T::Time> {
        let t = self.first_step_time();

        if t.index() == 0 {
            StepTime::Init
        } else {
            StepTime::Normal(t.raw_time())
        }
    }

    pub fn is_original(&self) -> bool {
        matches!(
            self.inner,
            StepInner::OriginalUnsaturated { .. } | StepInner::OriginalSaturating { .. }
        )
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

struct CurrentSaturating<'a, T: Transposer, S: StorageFamily> {
    sub_step: &'a mut SubStep<T, S>,
}

#[derive(Clone, Copy)]
pub enum StepTime<Time: Ord + Copy> {
    Init,
    Normal(Time),
}

impl<Time: Ord + Copy> Ord for StepTime<Time> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Self::Init, Self::Init) => std::cmp::Ordering::Equal,
            (Self::Init, Self::Normal(_)) => std::cmp::Ordering::Less,
            (Self::Normal(_), Self::Init) => std::cmp::Ordering::Greater,
            (Self::Normal(s), Self::Normal(o)) => s.cmp(o),
        }
    }
}

impl<Time: Ord + Copy> PartialOrd for StepTime<Time> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<Time: Ord + Copy> PartialEq for StepTime<Time> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl<Time: Ord + Copy> Eq for StepTime<Time> {}

enum StepInner<T: Transposer, S: StorageFamily, M: StepMetadata<T, S>> {
    OriginalUnsaturated {
        steps:    Vec<SubStep<T, S>>,
        metadata: M::OriginalUnsaturated,
    },
    OriginalSaturating {
        current_saturating_index: usize,
        steps:                    Vec<SubStep<T, S>>,
        metadata:                 M::OriginalSaturating,
    },
    RepeatUnsaturated {
        steps:    Box<[SubStep<T, S>]>,
        metadata: M::RepeatUnsaturated,
    },
    RepeatSaturating {
        current_saturating_index: usize,
        steps:                    Box<[SubStep<T, S>]>,
        metadata:                 M::RepeatSaturating,
    },
    Saturated {
        steps:    Box<[SubStep<T, S>]>,
        metadata: M::Saturated,
    },
    Unreachable,
}

#[derive(Debug, PartialEq, Eq)]
pub enum StepPoll<T: Transposer> {
    NeedsState,
    Emitted(T::Output),
    Pending,
    Ready,
}

pub enum Metadata<'a, T: Transposer, S: StorageFamily, M: StepMetadata<T, S>> {
    OriginalUnsaturated(&'a M::OriginalUnsaturated),
    OriginalSaturating(&'a M::OriginalSaturating),
    RepeatUnsaturated(&'a M::RepeatUnsaturated),
    RepeatSaturating(&'a M::RepeatSaturating),
    Saturated(&'a M::Saturated),
}

pub enum MetadataMut<'a, T: Transposer, S: StorageFamily, M: StepMetadata<T, S>> {
    OriginalUnsaturated(&'a mut M::OriginalUnsaturated),
    OriginalSaturating(&'a mut M::OriginalSaturating),
    RepeatUnsaturated(&'a mut M::RepeatUnsaturated),
    RepeatSaturating(&'a mut M::RepeatSaturating),
    Saturated(&'a mut M::Saturated),
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
