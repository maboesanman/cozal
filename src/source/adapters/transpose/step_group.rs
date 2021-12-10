use std::collections::HashMap;
use std::hint::unreachable_unchecked;
use std::sync::Weak;
use std::task::{Wake, Waker};

use futures_core::Future;

use crate::transposer::interpolation::Interpolation;
use crate::transposer::lazy_state::LazyState;
use crate::transposer::step::{PollErr as StepPollErr, SaturateErr, Step, StepPoll, StepTime};
use crate::transposer::Transposer;
use crate::util::stack_waker::StackWaker;
use crate::util::take_mut::{take_and_return_or_recover, take_or_recover};

pub struct StepGroup<T: Transposer> {
    inner:       StepGroupInner<T>,
    input_state: LazyState<T>,
    // these are used purely for enforcing that saturate calls use the previous wrapped_transposer.
    // #[cfg(debug_assertions)]
    // uuid_self: uuid::Uuid,
    // #[cfg(debug_assertions)]
    // uuid_prev: Option<uuid::Uuid>,
}

impl<T: Transposer> StepGroup<T> {
    pub fn new_init(transposer: T, rng_seed: [u8; 32]) -> Self {
        let mut steps = Vec::with_capacity(1);
        steps.push(Step::new_init(transposer, rng_seed));

        Self {
            inner: StepGroupInner::OriginalSaturating {
                current_saturating_index: 0,
                steps,
                waker_stack: StackWaker::new_empty(),
            },
        }
    }

    fn new_from_unsaturated(step: Step<T>) -> Self {
        Self {
            inner: StepGroupInner::OriginalUnsaturated {
                steps: vec![step]
            },
        }
    }

    // this only needs mut because it can remove interpolations.
    pub fn next_unsaturated(
        &mut self,
        next_inputs: &mut Option<(T::Time, Box<[T::Input]>)>,
    ) -> Result<Option<Self>, NextUnsaturatedErr> {
        let next_time: Option<T::Time> = next_inputs.as_ref().map(|(t, _)| *t);
        if let StepGroupInner::Saturated {
            interpolations, ..
        } = &mut self.inner
        {
            if let Some(next_time) = next_time {
                for (_, interpolation) in interpolations.drain_filter(|_, v| v.time() >= next_time)
                {
                    interpolation.wake();
                }
            }

            let next = self
                .final_step()
                .unwrap()
                .next_unsaturated(next_inputs)
                .map_err(|e| match e {
                    crate::transposer::step::NextUnsaturatedErr::NotSaturated => unreachable!(),
                    crate::transposer::step::NextUnsaturatedErr::InputPastOrPresent => {
                        NextUnsaturatedErr::InputPastOrPresent
                    },
                })?;

            Ok(next.map(|step| Self::new_from_unsaturated(step)))
        } else {
            Err(NextUnsaturatedErr::NotSaturated)
        }
    }

    pub fn poll(
        &mut self,
        channel: usize,
        waker: Waker,
    ) -> Result<(StepGroupPoll, Vec<T::Output>), PollErr> {
        let mut outputs = Vec::new();
        loop {
            let (step, waker_stack) = match self.current_saturating() {
                Ok(x) => x,
                Err(CurrentSaturatingErr::Unsaturated) => return Err(PollErr::Unsaturated),
                Err(CurrentSaturatingErr::Saturated) => return Ok((StepGroupPoll::Ready, outputs)),
            };

            let waker = match StackWaker::register(waker_stack, channel, waker.clone()) {
                Some(w) => Waker::from(w),
                None => break Ok((StepGroupPoll::Pending, outputs)),
            };

            let poll_result = step.poll(waker).map_err(|e| match e {
                StepPollErr::Unsaturated => PollErr::Unsaturated,
                StepPollErr::Saturated => PollErr::Saturated,
            })?;

            match poll_result {
                StepPoll::Pending => break Ok((StepGroupPoll::Pending, outputs)),
                StepPoll::NeedsState => break Ok((StepGroupPoll::NeedsState, outputs)),
                StepPoll::ReadyNoOutputs => {},
                StepPoll::ReadyOutputs(mut o) => outputs.append(&mut o),
            };

            if self.advance_saturation_index().unwrap() {
                break Ok((StepGroupPoll::Ready, outputs))
            }
        }
    }

    pub fn interpolate_poll(
        &mut self,
        time: T::Time,
        channel: usize,
        waker: Waker,
    ) -> Result<InterpolatePoll<T>, InterpolatePollErr> {
        #[cfg(debug_assertions)]
        if time < self.time() {
            return Err(InterpolatePollErr::TimePast)
        }

        // check if we already have an interpolation for this channel
        // if the time matches we can keep going with this.

        let (last_step, interpolations) = match &mut self.inner {
            StepGroupInner::Saturated {
                interpolations,
                steps,
            } => (steps.last().unwrap(), interpolations),
            StepGroupInner::Unreachable => unreachable!(),
            _ => return Err(InterpolatePollErr::NotSaturated),
        };
        let base_time = last_step.time().raw_time();
        let base = last_step.finished_wrapped_transposer().unwrap();
        let fut = T::interpolate(&base.transposer, base_time, time, todo!());
        // interpolations.insert(channel, v)

        todo!()
    }

    pub fn remove_interpolation(&mut self, channel: usize) {
        if let StepGroupInner::Saturated {
            interpolations, ..
        } = &mut self.inner
        {
            interpolations.remove(&channel);
        }
    }

    pub fn saturate_take(&mut self, prev: &mut Self) -> Result<(), SaturateTakeErr> {
        match (&mut prev.inner, &mut self.inner) {
            (
                StepGroupInner::OriginalUnsaturated {
                    ..
                }
                | StepGroupInner::OriginalSaturating {
                    ..
                }
                | StepGroupInner::RepeatUnsaturated {
                    ..
                }
                | StepGroupInner::RepeatSaturating {
                    ..
                },
                _,
            ) => Err(SaturateTakeErr::PreviousNotSaturated),
            (
                _,
                StepGroupInner::OriginalSaturating {
                    ..
                }
                | StepGroupInner::RepeatSaturating {
                    ..
                }
                | StepGroupInner::Saturated {
                    ..
                },
            ) => Err(SaturateTakeErr::SelfNotUnsaturated),
            (StepGroupInner::Unreachable, _) => unreachable!(),
            (_, StepGroupInner::Unreachable) => unreachable!(),
            (
                StepGroupInner::Saturated {
                    interpolations,
                    steps: prev_steps,
                },
                StepGroupInner::OriginalUnsaturated {
                    steps: self_steps,
                },
            ) => {
                if !interpolations.is_empty() {
                    Err(SaturateTakeErr::PreviousHasActiveInterpolations)
                } else {
                    self_steps
                        .first_mut()
                        .unwrap()
                        .saturate_take(prev_steps.last_mut().unwrap())
                        .map_err(|e| match e {
                            SaturateErr::PreviousNotSaturated => {
                                SaturateTakeErr::PreviousNotSaturated
                            },
                            SaturateErr::SelfNotUnsaturated => SaturateTakeErr::SelfNotUnsaturated,
                            #[cfg(debug_assertions)]
                            SaturateErr::IncorrectPrevious => SaturateTakeErr::IncorrectPrevious,
                        })?;
                    take_or_recover(
                        &mut prev.inner,
                        || StepGroupInner::Unreachable,
                        |inner| {
                            if let StepGroupInner::Saturated {
                                interpolations: _,
                                steps,
                            } = inner
                            {
                                StepGroupInner::RepeatUnsaturated {
                                    steps,
                                }
                            } else {
                                unsafe { unreachable_unchecked() }
                            }
                        },
                    );
                    take_or_recover(
                        &mut self.inner,
                        || StepGroupInner::Unreachable,
                        |inner| {
                            if let StepGroupInner::OriginalUnsaturated {
                                steps,
                            } = inner
                            {
                                StepGroupInner::OriginalSaturating {
                                    current_saturating_index: 0,
                                    steps,
                                    waker_stack: StackWaker::new_empty(),
                                }
                            } else {
                                unsafe { unreachable_unchecked() }
                            }
                        },
                    );
                    Ok(())
                }
            },
            (
                StepGroupInner::Saturated {
                    interpolations,
                    steps: prev_steps,
                },
                StepGroupInner::RepeatUnsaturated {
                    steps: self_steps,
                },
            ) => {
                if !interpolations.is_empty() {
                    Err(SaturateTakeErr::PreviousHasActiveInterpolations)
                } else {
                    self_steps
                        .first_mut()
                        .unwrap()
                        .saturate_take(prev_steps.last_mut().unwrap())
                        .map_err(|e| match e {
                            SaturateErr::PreviousNotSaturated => {
                                SaturateTakeErr::PreviousNotSaturated
                            },
                            SaturateErr::SelfNotUnsaturated => SaturateTakeErr::SelfNotUnsaturated,
                            #[cfg(debug_assertions)]
                            SaturateErr::IncorrectPrevious => SaturateTakeErr::IncorrectPrevious,
                        })?;
                    take_or_recover(
                        &mut prev.inner,
                        || StepGroupInner::Unreachable,
                        |inner| {
                            if let StepGroupInner::Saturated {
                                interpolations: _,
                                steps,
                            } = inner
                            {
                                StepGroupInner::RepeatUnsaturated {
                                    steps,
                                }
                            } else {
                                unsafe { unreachable_unchecked() }
                            }
                        },
                    );
                    take_or_recover(
                        &mut self.inner,
                        || StepGroupInner::Unreachable,
                        |inner| {
                            if let StepGroupInner::RepeatUnsaturated {
                                steps,
                            } = inner
                            {
                                StepGroupInner::RepeatSaturating {
                                    current_saturating_index: 0,
                                    steps,
                                    waker_stack: StackWaker::new_empty(),
                                }
                            } else {
                                unsafe { unreachable_unchecked() }
                            }
                        },
                    );
                    Ok(())
                }
            },
        }
    }

    pub fn saturate_clone(&mut self, prev: &Self) -> Result<(), SaturateCloneErr>
    where
        T: Clone,
    {
        match (&prev.inner, &mut self.inner) {
            (
                StepGroupInner::OriginalUnsaturated {
                    ..
                }
                | StepGroupInner::OriginalSaturating {
                    ..
                }
                | StepGroupInner::RepeatUnsaturated {
                    ..
                }
                | StepGroupInner::RepeatSaturating {
                    ..
                },
                _,
            ) => Err(SaturateCloneErr::PreviousNotSaturated),
            (
                _,
                StepGroupInner::OriginalSaturating {
                    ..
                }
                | StepGroupInner::RepeatSaturating {
                    ..
                }
                | StepGroupInner::Saturated {
                    ..
                },
            ) => Err(SaturateCloneErr::SelfNotUnsaturated),
            (StepGroupInner::Unreachable, _) => unreachable!(),
            (_, StepGroupInner::Unreachable) => unreachable!(),
            (
                StepGroupInner::Saturated {
                    interpolations: _,
                    steps: prev_steps,
                },
                StepGroupInner::OriginalUnsaturated {
                    steps: self_steps,
                },
            ) => {
                self_steps
                    .first_mut()
                    .unwrap()
                    .saturate_clone(prev_steps.last().unwrap())
                    .map_err(|e| match e {
                        SaturateErr::PreviousNotSaturated => SaturateCloneErr::PreviousNotSaturated,
                        SaturateErr::SelfNotUnsaturated => SaturateCloneErr::SelfNotUnsaturated,
                        #[cfg(debug_assertions)]
                        SaturateErr::IncorrectPrevious => SaturateCloneErr::IncorrectPrevious,
                    })?;
                take_or_recover(
                    &mut self.inner,
                    || StepGroupInner::Unreachable,
                    |inner| {
                        if let StepGroupInner::OriginalUnsaturated {
                            steps,
                        } = inner
                        {
                            StepGroupInner::OriginalSaturating {
                                current_saturating_index: 0,
                                steps,
                                waker_stack: StackWaker::new_empty(),
                            }
                        } else {
                            unsafe { unreachable_unchecked() }
                        }
                    },
                );
                Ok(())
            },
            (
                StepGroupInner::Saturated {
                    interpolations: _,
                    steps: prev_steps,
                },
                StepGroupInner::RepeatUnsaturated {
                    steps: self_steps,
                },
            ) => {
                self_steps
                    .first_mut()
                    .unwrap()
                    .saturate_clone(prev_steps.last().unwrap())
                    .map_err(|e| match e {
                        SaturateErr::PreviousNotSaturated => SaturateCloneErr::PreviousNotSaturated,
                        SaturateErr::SelfNotUnsaturated => SaturateCloneErr::SelfNotUnsaturated,
                        #[cfg(debug_assertions)]
                        SaturateErr::IncorrectPrevious => SaturateCloneErr::IncorrectPrevious,
                    })?;
                take_or_recover(
                    &mut self.inner,
                    || StepGroupInner::Unreachable,
                    |inner| {
                        if let StepGroupInner::RepeatUnsaturated {
                            steps,
                        } = inner
                        {
                            StepGroupInner::RepeatSaturating {
                                steps,
                                waker_stack: StackWaker::new_empty(),
                                current_saturating_index: 0,
                            }
                        } else {
                            unsafe { unreachable_unchecked() }
                        }
                    },
                );
                Ok(())
            },
        }
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
                    interpolations,
                    mut steps,
                } => {
                    steps.last_mut().unwrap().desaturate().unwrap();
                    if interpolations.is_empty() {
                        (
                            StepGroupInner::RepeatUnsaturated {
                                steps,
                            },
                            Ok(()),
                        )
                    } else {
                        (
                            StepGroupInner::Saturated {
                                interpolations,
                                steps,
                            },
                            Err(DesaturateErr::ActiveWakers),
                        )
                    }
                },
                StepGroupInner::Unreachable => unreachable!(),
            },
        )
    }

    pub fn set_input_state(&mut self, state: T::InputState) -> Result<(), T::InputState> {
        match self.current_saturating() {
            Ok((step, _)) => step.set_input_state(state),
            Err(_) => Err(state),
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
                steps, ..
            } => steps.first().unwrap().time(),
            StepGroupInner::Unreachable => unreachable!(),
        }
    }

    fn final_step(&self) -> Result<&Step<T>, ()> {
        match &self.inner {
            StepGroupInner::Saturated {
                steps, ..
            } => Ok(&steps.last().unwrap()),
            _ => Err(()),
        }
    }

    // this lives as long as this stepgroup is alive
    fn final_transposer(&self) -> Result<&T, ()> {
        let final_step = self.final_step()?;
        Ok(&final_step.finished_wrapped_transposer().unwrap().transposer)
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

    fn advance_saturation_index(&mut self) -> Result<bool, ()> {
        let (i, steps) = match &mut self.inner {
            StepGroupInner::OriginalSaturating {
                current_saturating_index: i,
                steps,
                ..
            } => {
                let next = steps
                    .last()
                    .unwrap()
                    .next_unsaturated_same_time()
                    .map_err(|_| ())?;

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
                    StepGroupInner::Saturated {
                        steps,
                        interpolations: HashMap::new(),
                    }
                },
            );
            return Ok(true)
        }

        *i += 1;
        let (part1, part2) = steps.split_at_mut(*i);
        let prev = part1.last_mut().unwrap();
        let next = part2.first_mut().unwrap();
        prev.saturate_take(next).unwrap();

        Ok(false)
    }

    fn is_saturated(&self) -> bool {
        matches!(self.inner, StepGroupInner::Saturated { .. })
    }

    fn is_desaturated(&self) -> bool {
        matches!(
            self.inner,
            StepGroupInner::OriginalSaturating { .. } | StepGroupInner::RepeatSaturating { .. }
        )
    }

    pub fn is_init(&self) -> bool {
        self.first_step_time().index() == 0
    }

    pub fn time(&self) -> T::Time {
        self.first_step_time().raw_time()
    }
}

impl<T: Transposer> Drop for StepGroup<T> {
    fn drop(&mut self) {
        if let StepGroupInner::Saturated {
            interpolations, ..
        } = &mut self.inner
        {
            for (_, i) in interpolations {
                i.wake();
            }
        }
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
    Saturated {
        // first because it has pointers into boxes owned by steps.
        interpolations: HashMap<usize, Interpolation<T>>,
        steps:          Box<[Step<T>]>,
    },
    Unreachable,
}

pub enum InterpolatePoll<T: Transposer> {
    Pending,
    NeedsState,
    Ready(T::OutputState),
}

pub enum StepGroupPoll {
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
