use std::collections::HashMap;
use std::hint::unreachable_unchecked;
use std::task::Waker;

use futures_core::Future;

use crate::transposer::step::{SaturateErr, Step, StepTime};
use crate::transposer::Transposer;
use crate::util::take_mut::{take_and_return_or_recover, take_or_recover};

pub struct StepGroup<T: Transposer> {
    inner: StepGroupInner<T>,
}

impl<T: Transposer> StepGroup<T> {
    pub fn new_init(transposer: T, rng_seed: [u8; 32]) -> Self {
        let mut steps = Vec::with_capacity(1);
        steps.push(Step::new_init(transposer, rng_seed));

        Self {
            inner: StepGroupInner::OriginalSaturating {
                current_saturating_index: 0,
                steps,
                wakers: HashMap::new(),
            },
        }
    }

    pub fn poll(
        &mut self,
        channel: usize,
        waker: Waker,
        next_inputs: &mut Option<(T::Time, Box<[T::Input]>)>,
    ) -> Result<StepGroupPoll<T>, ()> {
        unimplemented!()
    }

    pub fn interpolate_poll(
        &mut self,
        time: T::Time,
        channel: usize,
        waker: Waker,
    ) -> InterpolatePoll<T> {
        unimplemented!()
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
                                    wakers: HashMap::new(),
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
                                    wakers: HashMap::new(),
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
                                wakers: HashMap::new(),
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
                                wakers: HashMap::new(),
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
                    wakers,
                } => {
                    steps
                        .get_mut(current_saturating_index)
                        .unwrap()
                        .desaturate()
                        .unwrap();
                    if wakers.is_empty() {
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
                                wakers,
                            },
                            Err(DesaturateErr::ActiveWakers),
                        )
                    }
                },
                StepGroupInner::RepeatSaturating {
                    current_saturating_index,
                    mut steps,
                    wakers,
                } => {
                    steps
                        .get_mut(current_saturating_index)
                        .unwrap()
                        .desaturate()
                        .unwrap();
                    if wakers.is_empty() {
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
                                wakers,
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
            Ok(step) => step.set_input_state(state),
            Err(()) => Err(state),
        }
    }

    pub fn first_step_time(&self) -> &StepTime<T::Time> {
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

    // this lives as long as this stepgroup is alive
    fn final_transposer(&self) -> Option<&T> {
        match &self.inner {
            StepGroupInner::Saturated {
                steps, ..
            } => Some(
                &steps
                    .last()
                    .unwrap()
                    .finished_wrapped_transposer()
                    .unwrap()
                    .transposer,
            ),
            _ => None,
        }
    }

    fn current_saturating(&mut self) -> Result<&mut Step<T>, ()> {
        match &mut self.inner {
            StepGroupInner::OriginalSaturating {
                current_saturating_index,
                steps,
                ..
            } => steps.get_mut(*current_saturating_index).ok_or(()),
            StepGroupInner::RepeatSaturating {
                current_saturating_index,
                steps,
                ..
            } => steps.get_mut(*current_saturating_index).ok_or(()),
            _ => Err(()),
        }
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
}

enum StepGroupInner<T: Transposer> {
    OriginalUnsaturated {
        steps: Vec<Step<T>>,
    },
    OriginalSaturating {
        current_saturating_index: usize,
        steps:                    Vec<Step<T>>,
        wakers:                   HashMap<usize, Waker>,
    },
    RepeatUnsaturated {
        steps: Box<[Step<T>]>,
    },
    RepeatSaturating {
        current_saturating_index: usize,
        steps:                    Box<[Step<T>]>,
        wakers:                   HashMap<usize, Waker>,
    },
    Saturated {
        // first because it has pointers into boxes owned by steps.
        interpolations: HashMap<usize, Interpolation<T>>,
        steps:          Box<[Step<T>]>,
    },
    Unreachable,
}

struct Interpolation<T: Transposer> {
    time:   T::Time,
    future: Box<dyn Future<Output = T::OutputState>>,
}

pub enum StepGroupPoll<T: Transposer> {
    Pending,
    NeedsState,
    PendingOutputs {
        outputs: Vec<T::Output>,
    },
    ReadyNoOutputs {
        next: StepGroup<T>,
    },
    ReadyOutputs {
        outputs: Vec<T::Output>,
        next:    StepGroup<T>,
    },
    ReadyAgain,
}

pub enum InterpolatePoll<T: Transposer> {
    Pending,
    NeedsState,
    Ready(T::OutputState),
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
