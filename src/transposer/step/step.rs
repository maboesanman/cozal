use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use futures_core::Future;

use super::interpolation::Interpolation;
use super::lazy_state::LazyState;
use super::pointer_interpolation::PointerInterpolation;
use super::sub_step::{PollErr as StepPollErr, SubStep, SubStepTime};
use crate::transposer::schedule_storage::{ImArcStorage, StorageFamily};
use crate::transposer::step::sub_step::SaturateErr;
use crate::transposer::Transposer;
use crate::util::take_mut::{take_and_return_or_recover, take_or_recover};

pub struct Step<T: Transposer, S: StorageFamily = ImArcStorage> {
    inner: StepInner<T, S>,

    // boxed to make self reference easier.
    input_state: Box<LazyState<T::InputState>>,

    // these are used purely for enforcing that saturate calls use the previous step_group.
    #[cfg(debug_assertions)]
    uuid_self: uuid::Uuid,
    #[cfg(debug_assertions)]
    uuid_prev: Option<uuid::Uuid>,
}

pub type NextInputs<T> = Option<(<T as Transposer>::Time, Box<[<T as Transposer>::Input]>)>;

impl<T: Transposer, S: StorageFamily> Step<T, S> {
    pub fn new_init(transposer: T, rng_seed: [u8; 32]) -> Self {
        let mut steps = Vec::with_capacity(1);
        let input_state = Box::new(LazyState::new());
        let input_state_ptr = input_state.as_ref();

        // SAFETY: steps are dropped before input_state.
        steps.push(unsafe { SubStep::new_init(transposer, rng_seed, input_state_ptr) });

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

    // this only needs mut because it can remove interpolations.
    pub fn next_unsaturated(
        &mut self,
        next_inputs: &mut NextInputs<T>,
    ) -> Result<Option<Self>, NextUnsaturatedErr> {
        if let StepInner::Saturated {
            steps,
        } = &mut self.inner
        {
            let input_state = Box::new(LazyState::new());
            let input_state_ptr = input_state.as_ref();

            let next = steps
                .last()
                .unwrap()
                .next_unsaturated(next_inputs, input_state_ptr)
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

    pub fn saturate_take(&mut self, prev: &mut Self) -> Result<(), SaturateTakeErr> {
        #[cfg(debug_assertions)]
        if self.uuid_prev != Some(prev.uuid_self) {
            return Err(SaturateTakeErr::IncorrectPrevious)
        }

        enum TakeFromPreviousErr {
            SaturateErr(SaturateErr),
            PreviousHasActiveInterpolations,
        }

        fn take_from_previous<T: Transposer, S: StorageFamily>(
            prev: &mut StepInner<T, S>,
            next: &mut SubStep<T, S>,
        ) -> Result<(), TakeFromPreviousErr> {
            take_and_return_or_recover(
                prev,
                || StepInner::Unreachable,
                |inner| {
                    if let StepInner::Saturated {
                        mut steps,
                    } = inner
                    {
                        match next.saturate_take(steps.last_mut().unwrap()) {
                            Err(err) => {
                                let replacement = StepInner::Saturated {
                                    steps,
                                };
                                (replacement, Err(TakeFromPreviousErr::SaturateErr(err)))
                            },
                            Ok(()) => {
                                let replacement = StepInner::RepeatUnsaturated {
                                    steps,
                                };

                                (replacement, Ok(()))
                            },
                        }
                    } else {
                        (
                            inner,
                            Err(TakeFromPreviousErr::SaturateErr(
                                SaturateErr::PreviousNotSaturated,
                            )),
                        )
                    }
                },
            )
        }

        self.saturate(|next| take_from_previous(&mut prev.inner, next))
            .map_err(|err| match err {
                Some(TakeFromPreviousErr::PreviousHasActiveInterpolations) => {
                    SaturateTakeErr::PreviousHasActiveInterpolations
                },
                Some(TakeFromPreviousErr::SaturateErr(e)) => match e {
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
    {
        #[cfg(debug_assertions)]
        if self.uuid_prev != Some(prev.uuid_self) {
            return Err(SaturateCloneErr::IncorrectPrevious)
        }

        fn clone_from_previous<T: Transposer, S: StorageFamily>(
            prev: &StepInner<T, S>,
            next: &mut SubStep<T, S>,
        ) -> Result<(), SaturateErr>
        where
            T: Clone,
        {
            if let StepInner::Saturated {
                steps,
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
        take_and_return_or_recover(
            &mut self.inner,
            || StepInner::Unreachable,
            |inner| match inner {
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
                StepInner::Unreachable => unreachable!(),
                _ => (inner, Err(None)),
            },
        )
    }

    pub fn desaturate(&mut self) -> Result<(), DesaturateErr> {
        take_and_return_or_recover(
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
                StepInner::Unreachable => unreachable!(),
            },
        )
    }

    pub fn poll(&mut self, waker: Waker) -> Result<StepPoll<T>, PollErr> {
        let mut outputs = Vec::new();
        loop {
            let CurrentSaturating {
                step,
            } = match self.current_saturating() {
                Ok(x) => x,
                Err(CurrentSaturatingErr::Unsaturated) => return Err(PollErr::Unsaturated),
                Err(CurrentSaturatingErr::Saturated) => return Ok(StepPoll::new_ready(outputs)),
            };
            let step = Pin::new(step);
            let mut cx = Context::from_waker(&waker);
            let poll_result = step.poll(&mut cx).map_err(|e| match e {
                StepPollErr::Unsaturated => PollErr::Unsaturated,
                StepPollErr::Saturated => PollErr::Saturated,
            })?;

            match poll_result {
                Poll::Pending => {
                    break Ok(if self.input_state.requested() {
                        StepPoll::new_needs_state(outputs)
                    } else {
                        StepPoll::new_pending(outputs)
                    })
                },
                Poll::Ready(Some(mut o)) => outputs.append(&mut o),
                Poll::Ready(None) => {},
            };

            // now we are ready, we need to advance to the next sub-step.
            if self.advance_saturation_index().is_saturated() {
                break Ok(StepPoll::new_ready(outputs))
            }
        }
    }

    pub fn set_input_state(
        &mut self,
        state: T::InputState,
        ignore_waker: &Waker,
    ) -> Result<(), Box<T::InputState>> {
        self.input_state.set(state, ignore_waker)
    }

    pub(crate) fn interpolate_pointer(
        &self,
        time: T::Time,
    ) -> Result<PointerInterpolation<T>, InterpolateErr> {
        match &self.inner {
            StepInner::Saturated {
                steps,
            } => {
                #[cfg(debug_assertions)]
                if self.raw_time() > time {
                    return Err(InterpolateErr::TimePast)
                }

                Ok(unsafe {
                    PointerInterpolation::new(
                        time,
                        steps
                            .as_ref()
                            .last()
                            .unwrap()
                            .finished_wrapped_transposer()
                            .unwrap(),
                    )
                })
            },
            _ => Err(InterpolateErr::NotSaturated),
        }
    }

    pub fn interpolate(&self, time: T::Time) -> Result<Interpolation<'_, T, S>, InterpolateErr> {
        match &self.inner {
            StepInner::Saturated {
                steps,
            } => {
                #[cfg(debug_assertions)]
                if self.raw_time() > time {
                    return Err(InterpolateErr::TimePast)
                }

                Ok(unsafe {
                    Interpolation::new(
                        time,
                        steps
                            .as_ref()
                            .last()
                            .unwrap()
                            .finished_wrapped_transposer()
                            .unwrap(),
                    )
                })
            },
            _ => Err(InterpolateErr::NotSaturated),
        }
    }

    fn first_step_time(&self) -> &SubStepTime<T::Time> {
        match &self.inner {
            StepInner::OriginalUnsaturated {
                steps, ..
            } => steps.first().unwrap().time(),
            StepInner::RepeatUnsaturated {
                steps,
            } => steps.first().unwrap().time(),
            StepInner::OriginalSaturating {
                steps, ..
            } => steps.first().unwrap().time(),
            StepInner::RepeatSaturating {
                steps, ..
            } => steps.first().unwrap().time(),
            StepInner::Saturated {
                steps,
            } => steps.first().unwrap().time(),
            StepInner::Unreachable => unreachable!(),
        }
    }

    fn current_saturating(&mut self) -> Result<CurrentSaturating<T, S>, CurrentSaturatingErr> {
        match &mut self.inner {
            StepInner::OriginalSaturating {
                current_saturating_index,
                steps,
            } => Ok(CurrentSaturating {
                step: steps.get_mut(*current_saturating_index).unwrap(),
            }),
            StepInner::RepeatSaturating {
                current_saturating_index,
                steps,
            } => Ok(CurrentSaturating {
                step: steps.get_mut(*current_saturating_index).unwrap(),
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
            take_or_recover(
                &mut self.inner,
                || StepInner::Unreachable,
                |inner| {
                    let steps: Box<[SubStep<T, S>]> = match inner {
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

    fn is_saturated(&self) -> bool {
        matches!(self.inner, StepInner::Saturated { .. })
    }

    fn is_unsaturated(&self) -> bool {
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
    step: &'a mut SubStep<T, S>,
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

enum StepInner<T: Transposer, S: StorageFamily> {
    OriginalUnsaturated {
        steps: Vec<SubStep<T, S>>,
    },
    OriginalSaturating {
        current_saturating_index: usize,
        steps:                    Vec<SubStep<T, S>>,
    },
    RepeatUnsaturated {
        steps: Box<[SubStep<T, S>]>,
    },
    RepeatSaturating {
        current_saturating_index: usize,
        steps:                    Box<[SubStep<T, S>]>,
    },
    Saturated {
        steps: Box<[SubStep<T, S>]>,
    },
    Unreachable,
}

pub struct StepPoll<T: Transposer> {
    pub result:  StepPollResult,
    pub outputs: Vec<T::Output>,
}

impl<T: Transposer> StepPoll<T> {
    pub fn new_pending(outputs: Vec<T::Output>) -> Self {
        Self {
            result: StepPollResult::Pending,
            outputs,
        }
    }
    pub fn new_needs_state(outputs: Vec<T::Output>) -> Self {
        Self {
            result: StepPollResult::NeedsState,
            outputs,
        }
    }
    pub fn new_ready(outputs: Vec<T::Output>) -> Self {
        Self {
            result: StepPollResult::Ready,
            outputs,
        }
    }
}

#[derive(Debug)]
pub enum StepPollResult {
    NeedsState,
    Pending,
    Ready,
}

#[derive(Debug)]
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
