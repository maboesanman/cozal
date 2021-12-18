use std::pin::Pin;
use std::sync::Weak;
use std::task::{Context, Poll, Waker};

use futures_core::Future;

use super::interpolation::Interpolation;
use super::lazy_state::LazyState;
use super::pointer_interpolation::PointerInterpolation;
use super::step::{PollErr as StepPollErr, Step, StepTime};
use crate::transposer::schedule_storage::{ImArcStorage, StorageFamily};
use crate::transposer::step_group::step::SaturateErr;
use crate::transposer::Transposer;
use crate::util::stack_waker::StackWaker;
use crate::util::take_mut::{take_and_return_or_recover, take_or_recover};

pub struct StepGroup<T: Transposer, S: StorageFamily = ImArcStorage> {
    inner:       StepGroupInner<T, S>,
    input_state: Box<LazyState<T::InputState>>,

    // these are used purely for enforcing that saturate calls use the previous step_group.
    #[cfg(debug_assertions)]
    uuid_self: uuid::Uuid,
    #[cfg(debug_assertions)]
    uuid_prev: Option<uuid::Uuid>,
}

pub type NextInputs<T> = Option<(<T as Transposer>::Time, Box<[<T as Transposer>::Input]>)>;

impl<T: Transposer, S: StorageFamily> StepGroup<T, S> {
    pub fn new_init(transposer: T, rng_seed: [u8; 32]) -> Self {
        let mut steps = Vec::with_capacity(1);
        let input_state = Box::new(LazyState::new());
        let input_state_ptr = input_state.as_ref();

        // SAFETY: steps are dropped before input_state.
        steps.push(unsafe { Step::new_init(transposer, rng_seed, input_state_ptr) });

        Self {
            inner: StepGroupInner::OriginalSaturating {
                current_saturating_index: 0,
                steps,
                waker_stack: StackWaker::new_empty(),
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
        if let StepGroupInner::Saturated {
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
                    super::step::NextUnsaturatedErr::NotSaturated => unreachable!(),

                    #[cfg(debug_assertions)]
                    super::step::NextUnsaturatedErr::InputPastOrPresent => {
                        NextUnsaturatedErr::InputPastOrPresent
                    },
                })?;

            let next = next.map(|step| StepGroup {
                inner: StepGroupInner::OriginalUnsaturated {
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
            prev: &mut StepGroupInner<T, S>,
            next: &mut Step<T, S>,
        ) -> Result<(), TakeFromPreviousErr> {
            take_and_return_or_recover(
                prev,
                || StepGroupInner::Unreachable,
                |inner| {
                    if let StepGroupInner::Saturated {
                        mut steps,
                    } = inner
                    {
                        match next.saturate_take(steps.last_mut().unwrap()) {
                            Err(err) => {
                                let replacement = StepGroupInner::Saturated {
                                    steps,
                                };
                                (replacement, Err(TakeFromPreviousErr::SaturateErr(err)))
                            },
                            Ok(()) => {
                                let replacement = StepGroupInner::RepeatUnsaturated {
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
            prev: &StepGroupInner<T, S>,
            next: &mut Step<T, S>,
        ) -> Result<(), SaturateErr>
        where
            T: Clone,
        {
            if let StepGroupInner::Saturated {
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
        F: FnOnce(&mut Step<T, S>) -> Result<(), E>,
    {
        take_and_return_or_recover(
            &mut self.inner,
            || StepGroupInner::Unreachable,
            |inner| match inner {
                StepGroupInner::OriginalUnsaturated {
                    mut steps,
                } => match saturate_first_step(steps.first_mut().unwrap()) {
                    Ok(()) => {
                        let replacement = StepGroupInner::OriginalSaturating {
                            current_saturating_index: 0,
                            steps,
                            waker_stack: StackWaker::new_empty(),
                        };
                        (replacement, Ok(()))
                    },
                    Err(e) => (
                        StepGroupInner::OriginalUnsaturated {
                            steps,
                        },
                        Err(Some(e)),
                    ),
                },
                StepGroupInner::RepeatUnsaturated {
                    mut steps,
                } => match saturate_first_step(steps.first_mut().unwrap()) {
                    Ok(()) => {
                        let replacement = StepGroupInner::RepeatSaturating {
                            current_saturating_index: 0,
                            steps,
                            waker_stack: StackWaker::new_empty(),
                        };
                        (replacement, Ok(()))
                    },
                    Err(e) => (
                        StepGroupInner::RepeatUnsaturated {
                            steps,
                        },
                        Err(Some(e)),
                    ),
                },
                StepGroupInner::Unreachable => unreachable!(),
                _ => (inner, Err(None)),
            },
        )
    }

    pub fn desaturate(&mut self) -> Result<(), DesaturateErr> {
        take_and_return_or_recover(
            &mut self.inner,
            || StepGroupInner::Unreachable,
            |inner| match inner {
                StepGroupInner::OriginalUnsaturated {
                    ..
                }
                | StepGroupInner::RepeatUnsaturated {
                    ..
                } => (inner, Err(DesaturateErr::AlreadyUnsaturated)),
                StepGroupInner::OriginalSaturating {
                    current_saturating_index,
                    mut steps,
                    waker_stack,
                } => {
                    steps
                        .get_mut(current_saturating_index)
                        .unwrap()
                        .desaturate()
                        .unwrap();
                    if StackWaker::is_empty(&waker_stack) {
                        (
                            StepGroupInner::OriginalUnsaturated {
                                steps,
                            },
                            Ok(()),
                        )
                    } else {
                        (
                            StepGroupInner::OriginalSaturating {
                                current_saturating_index,
                                steps,
                                waker_stack,
                            },
                            Err(DesaturateErr::ActiveWakers),
                        )
                    }
                },
                StepGroupInner::RepeatSaturating {
                    current_saturating_index,
                    mut steps,
                    waker_stack,
                } => {
                    steps
                        .get_mut(current_saturating_index)
                        .unwrap()
                        .desaturate()
                        .unwrap();
                    if StackWaker::is_empty(&waker_stack) {
                        (
                            StepGroupInner::RepeatUnsaturated {
                                steps,
                            },
                            Ok(()),
                        )
                    } else {
                        (
                            StepGroupInner::RepeatSaturating {
                                current_saturating_index,
                                steps,
                                waker_stack,
                            },
                            Err(DesaturateErr::ActiveWakers),
                        )
                    }
                },
                StepGroupInner::Saturated {
                    steps,
                } => (
                    StepGroupInner::RepeatUnsaturated {
                        steps,
                    },
                    Ok(()),
                ),
                StepGroupInner::Unreachable => unreachable!(),
            },
        )
    }

    pub fn poll_progress(
        &mut self,
        channel: usize,
        waker: Waker,
    ) -> Result<StepGroupPoll<T>, PollErr> {
        let mut outputs = Vec::new();
        loop {
            let CurrentSaturating {
                step,
                waker_stack,
            } = match self.current_saturating() {
                Ok(x) => x,
                Err(CurrentSaturatingErr::Unsaturated) => return Err(PollErr::Unsaturated),
                Err(CurrentSaturatingErr::Saturated) => {
                    return Ok(StepGroupPoll::new_ready(outputs))
                },
            };

            let waker = match StackWaker::register(waker_stack, channel, waker.clone()) {
                Some(w) => Waker::from(w),
                None => break Ok(StepGroupPoll::new_pending(outputs)),
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
                        StepGroupPoll::new_needs_state(outputs)
                    } else {
                        StepGroupPoll::new_pending(outputs)
                    })
                },
                Poll::Ready(Some(mut o)) => outputs.append(&mut o),
                Poll::Ready(None) => {},
            };

            if self.advance_saturation_index() {
                break Ok(StepGroupPoll::new_ready(outputs))
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
            StepGroupInner::Saturated {
                steps,
            } => {
                if self.raw_time() > time {
                    Err(InterpolateErr::TimePast)
                } else {
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
                }
            },
            _ => Err(InterpolateErr::NotSaturated),
        }
    }

    pub fn interpolate(&self, time: T::Time) -> Result<Interpolation<'_, T, S>, InterpolateErr> {
        match &self.inner {
            StepGroupInner::Saturated {
                steps,
            } => {
                if self.raw_time() > time {
                    Err(InterpolateErr::TimePast)
                } else {
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
                }
            },
            _ => Err(InterpolateErr::NotSaturated),
        }
    }

    fn first_step_time(&self) -> &StepTime<T::Time> {
        match &self.inner {
            StepGroupInner::OriginalUnsaturated {
                steps, ..
            } => steps.first().unwrap().time(),
            StepGroupInner::RepeatUnsaturated {
                steps,
            } => steps.first().unwrap().time(),
            StepGroupInner::OriginalSaturating {
                steps, ..
            } => steps.first().unwrap().time(),
            StepGroupInner::RepeatSaturating {
                steps, ..
            } => steps.first().unwrap().time(),
            StepGroupInner::Saturated {
                steps,
            } => steps.first().unwrap().time(),
            StepGroupInner::Unreachable => unreachable!(),
        }
    }

    fn current_saturating(&mut self) -> Result<CurrentSaturating<T, S>, CurrentSaturatingErr> {
        match &mut self.inner {
            StepGroupInner::OriginalSaturating {
                current_saturating_index,
                steps,
                waker_stack,
            } => Ok(CurrentSaturating {
                step: steps.get_mut(*current_saturating_index).unwrap(),
                waker_stack,
            }),
            StepGroupInner::RepeatSaturating {
                current_saturating_index,
                steps,
                waker_stack,
            } => Ok(CurrentSaturating {
                step: steps.get_mut(*current_saturating_index).unwrap(),
                waker_stack,
            }),
            StepGroupInner::OriginalUnsaturated {
                ..
            } => Err(CurrentSaturatingErr::Unsaturated),
            StepGroupInner::RepeatUnsaturated {
                ..
            } => Err(CurrentSaturatingErr::Unsaturated),
            StepGroupInner::Saturated {
                ..
            } => Err(CurrentSaturatingErr::Saturated),
            StepGroupInner::Unreachable => unreachable!(),
        }
    }

    fn advance_saturation_index(&mut self) -> bool {
        let (i, steps) = match &mut self.inner {
            StepGroupInner::OriginalSaturating {
                current_saturating_index: i,
                steps,
                ..
            } => {
                let next = steps.last().unwrap().next_unsaturated_same_time().unwrap();

                if let Some(next) = next {
                    steps.push(next);
                }

                (i, steps.as_mut_slice())
            },
            StepGroupInner::RepeatSaturating {
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
                || StepGroupInner::Unreachable,
                |inner| {
                    let steps: Box<[Step<T, S>]> = match inner {
                        StepGroupInner::OriginalSaturating {
                            steps, ..
                        } => steps.into_boxed_slice(),
                        StepGroupInner::RepeatSaturating {
                            steps, ..
                        } => steps,
                        _ => unreachable!(),
                    };
                    StepGroupInner::Saturated {
                        steps,
                    }
                },
            );
            return true
        }

        *i += 1;
        let (part1, part2) = steps.split_at_mut(*i);
        let prev = part1.last_mut().unwrap();
        let next = part2.first_mut().unwrap();
        prev.saturate_take(next).unwrap();

        false
    }

    fn is_saturated(&self) -> bool {
        matches!(self.inner, StepGroupInner::Saturated { .. })
    }

    fn is_unsaturated(&self) -> bool {
        matches!(
            self.inner,
            StepGroupInner::OriginalUnsaturated { .. } | StepGroupInner::RepeatUnsaturated { .. }
        )
    }

    pub fn is_init(&self) -> bool {
        self.first_step_time().index() == 0
    }

    pub fn raw_time(&self) -> T::Time {
        self.first_step_time().raw_time()
    }

    pub fn time(&self) -> StepGroupTime<T::Time> {
        let t = self.first_step_time();

        if t.index() == 0 {
            StepGroupTime::Init
        } else {
            StepGroupTime::Normal(t.raw_time())
        }
    }
}

struct CurrentSaturating<'a, T: Transposer, S: StorageFamily> {
    step:        &'a mut Step<T, S>,
    waker_stack: &'a mut Weak<StackWaker<usize>>,
}

#[derive(Clone, Copy)]
pub enum StepGroupTime<Time: Ord + Copy> {
    Init,
    Normal(Time),
}

impl<Time: Ord + Copy> Ord for StepGroupTime<Time> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Self::Init, Self::Init) => std::cmp::Ordering::Equal,
            (Self::Init, Self::Normal(_)) => std::cmp::Ordering::Less,
            (Self::Normal(_), Self::Init) => std::cmp::Ordering::Greater,
            (Self::Normal(s), Self::Normal(o)) => s.cmp(o),
        }
    }
}

impl<Time: Ord + Copy> PartialOrd for StepGroupTime<Time> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<Time: Ord + Copy> PartialEq for StepGroupTime<Time> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl<Time: Ord + Copy> Eq for StepGroupTime<Time> {}

enum StepGroupInner<T: Transposer, S: StorageFamily> {
    OriginalUnsaturated {
        steps: Vec<Step<T, S>>,
    },
    OriginalSaturating {
        current_saturating_index: usize,
        steps:                    Vec<Step<T, S>>,
        waker_stack:              Weak<StackWaker<usize>>,
    },
    RepeatUnsaturated {
        steps: Box<[Step<T, S>]>,
    },
    RepeatSaturating {
        current_saturating_index: usize,
        steps:                    Box<[Step<T, S>]>,
        waker_stack:              Weak<StackWaker<usize>>,
    },
    Saturated {
        steps: Box<[Step<T, S>]>,
    },
    Unreachable,
}

pub enum InterpolatePoll<T: Transposer> {
    Pending,
    NeedsState,
    Ready(T::OutputState),
}

pub struct StepGroupPoll<T: Transposer> {
    pub result:  StepGroupPollResult,
    pub outputs: Vec<T::Output>,
}

impl<T: Transposer> StepGroupPoll<T> {
    pub fn new_pending(outputs: Vec<T::Output>) -> Self {
        Self {
            result: StepGroupPollResult::Pending,
            outputs,
        }
    }
    pub fn new_needs_state(outputs: Vec<T::Output>) -> Self {
        Self {
            result: StepGroupPollResult::NeedsState,
            outputs,
        }
    }
    pub fn new_ready(outputs: Vec<T::Output>) -> Self {
        Self {
            result: StepGroupPollResult::Ready,
            outputs,
        }
    }
}

#[derive(Debug)]
pub enum StepGroupPollResult {
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
