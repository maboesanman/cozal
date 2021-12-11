use std::pin::Pin;
use std::sync::Weak;
use std::task::{Context, Poll, Waker};

use futures_core::Future;

use super::lazy_state::LazyState;
use super::step::{PollErr as StepPollErr, Step, StepTime};
use super::step_group_saturated::StepGroupSaturated;
use crate::transposer::step_group::step::SaturateErr;
use crate::transposer::Transposer;
use crate::util::stack_waker::StackWaker;
use crate::util::take_mut::{take_and_return_or_recover, take_or_recover};

pub struct StepGroup<T: Transposer> {
    inner:       StepGroupInner<T>,
    input_state: Box<LazyState<T::InputState>>,
    // these are used purely for enforcing that saturate calls use the previous wrapped_transposer.
    #[cfg(debug_assertions)]
    uuid_self:   uuid::Uuid,
    #[cfg(debug_assertions)]
    uuid_prev:   Option<uuid::Uuid>,
}

impl<T: Transposer> StepGroup<T> {
    pub fn new_init(transposer: T, rng_seed: [u8; 32]) -> Self {
        let mut steps = Vec::with_capacity(1);
        let input_state = Box::new(LazyState::new());
        let input_state_ptr = input_state.as_ref();
        steps.push(Step::new_init(transposer, rng_seed, input_state_ptr));

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
        next_inputs: &mut Option<(T::Time, Box<[T::Input]>)>,
    ) -> Result<Option<Self>, NextUnsaturatedErr> {
        if let StepGroupInner::Saturated(saturated) = &mut self.inner {
            let input_state = Box::new(LazyState::new());
            let input_state_ptr = input_state.as_ref();

            let next = saturated.next_unsaturated(next_inputs, input_state_ptr)?;
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

        fn take_from_previous<T: Transposer>(
            prev: &mut StepGroupInner<T>,
            next: &mut Step<T>,
        ) -> Result<(), TakeFromPreviousErr> {
            take_and_return_or_recover(
                prev,
                || StepGroupInner::Unreachable,
                |inner| {
                    if let StepGroupInner::Saturated(saturated) = inner {
                        match saturated.take() {
                            Ok(mut prev_steps) => {
                                match next.saturate_take(prev_steps.last_mut().unwrap()) {
                                    Err(err) => {
                                        let replacement = StepGroupInner::Saturated(
                                            StepGroupSaturated::new(prev_steps),
                                        );
                                        (replacement, Err(TakeFromPreviousErr::SaturateErr(err)))
                                    },
                                    Ok(()) => {
                                        let replacement = StepGroupInner::RepeatUnsaturated {
                                            steps: prev_steps,
                                        };

                                        (replacement, Ok(()))
                                    },
                                }
                            },
                            Err(saturated) => {
                                let replacement = StepGroupInner::Saturated(saturated);

                                (
                                    replacement,
                                    Err(TakeFromPreviousErr::PreviousHasActiveInterpolations),
                                )
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

        fn clone_from_previous<T: Transposer>(
            prev: &StepGroupInner<T>,
            next: &mut Step<T>,
        ) -> Result<(), SaturateErr>
        where
            T: Clone,
        {
            if let StepGroupInner::Saturated(saturated) = prev {
                let last_step = saturated.final_step();
                next.saturate_clone(last_step)
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
        F: FnOnce(&mut Step<T>) -> Result<(), E>,
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
                StepGroupInner::Saturated(saturated) => match saturated.take() {
                    Ok(steps) => (
                        StepGroupInner::RepeatUnsaturated {
                            steps,
                        },
                        Ok(()),
                    ),
                    Err(saturated) => (
                        StepGroupInner::Saturated(saturated),
                        Err(DesaturateErr::ActiveInterpolations),
                    ),
                },
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
            let (step, waker_stack) = match self.current_saturating() {
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
            let step = unsafe { Pin::new_unchecked(step) };
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

    pub fn poll_interpolate(
        &mut self,
        time: T::Time,
        channel: usize,
        waker: Waker,
    ) -> Result<InterpolatePoll<T>, InterpolatePollErr> {
        if let StepGroupInner::Saturated(saturated) = &mut self.inner {
            saturated.poll_interpolate(time, channel, waker)
        } else {
            Err(InterpolatePollErr::NotSaturated)
        }
    }

    pub fn set_input_state(&mut self, state: T::InputState) -> Result<(), Box<T::InputState>> {
        self.input_state.set(state)
    }

    pub fn set_interpolation_input_state(
        &mut self,
        time: T::Time,
        channel: usize,
        state: T::InputState,
    ) -> Result<(), Box<T::InputState>> {
        if let StepGroupInner::Saturated(saturated) = &mut self.inner {
            saturated.set_interpolation_input_state(time, channel, state)
        } else {
            Err(Box::new(state))
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
            StepGroupInner::Saturated(s) => s.first_time(),
            StepGroupInner::Unreachable => unreachable!(),
        }
    }

    fn current_saturating(
        &mut self,
    ) -> Result<(&mut Step<T>, &mut Weak<StackWaker<usize>>), CurrentSaturatingErr> {
        match &mut self.inner {
            StepGroupInner::OriginalSaturating {
                current_saturating_index,
                steps,
                waker_stack,
            } => Ok((
                steps.get_mut(*current_saturating_index).unwrap(),
                waker_stack,
            )),
            StepGroupInner::RepeatSaturating {
                current_saturating_index,
                steps,
                waker_stack,
            } => Ok((
                steps.get_mut(*current_saturating_index).unwrap(),
                waker_stack,
            )),
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
                    let steps: Box<[Step<T>]> = match inner {
                        StepGroupInner::OriginalSaturating {
                            steps, ..
                        } => steps.into_boxed_slice(),
                        StepGroupInner::RepeatSaturating {
                            steps, ..
                        } => steps,
                        _ => unreachable!(),
                    };
                    StepGroupInner::Saturated(StepGroupSaturated::new(steps))
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

    pub fn time(&self) -> T::Time {
        self.first_step_time().raw_time()
    }
}

enum StepGroupInner<T: Transposer> {
    OriginalUnsaturated {
        steps: Vec<Step<T>>,
    },
    OriginalSaturating {
        current_saturating_index: usize,
        steps:                    Vec<Step<T>>,
        waker_stack:              Weak<StackWaker<usize>>,
    },
    RepeatUnsaturated {
        steps: Box<[Step<T>]>,
    },
    RepeatSaturating {
        current_saturating_index: usize,
        steps:                    Box<[Step<T>]>,
        waker_stack:              Weak<StackWaker<usize>>,
    },
    Saturated(StepGroupSaturated<T>),
    Unreachable,
}

pub enum InterpolatePoll<T: Transposer> {
    Pending,
    NeedsState,
    Ready(T::OutputState),
}

pub struct StepGroupPoll<T: Transposer> {
    result:  StepGroupPollResult,
    outputs: Vec<T::Output>,
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

pub enum StepGroupPollResult {
    NeedsState,
    Pending,
    Ready,
}

pub enum PollErr {
    Unsaturated,
    Saturated,
}

pub enum InterpolatePollErr {
    NotSaturated,
    #[cfg(debug_assertions)]
    TimePast,
}

enum CurrentSaturatingErr {
    Unsaturated,
    Saturated,
}

pub enum NextUnsaturatedErr {
    NotSaturated,
    #[cfg(debug_assertions)]
    InputPastOrPresent,
}

pub enum SaturateTakeErr {
    PreviousNotSaturated,
    SelfNotUnsaturated,
    #[cfg(debug_assertions)]
    IncorrectPrevious,
    PreviousHasActiveInterpolations,
}

pub enum SaturateCloneErr {
    PreviousNotSaturated,
    SelfNotUnsaturated,
    #[cfg(debug_assertions)]
    IncorrectPrevious,
}

pub enum DesaturateErr {
    AlreadyUnsaturated,
    ActiveWakers,
    ActiveInterpolations,
}
