use core::pin::Pin;
use core::task::{Context, Waker};
use std::collections::{BTreeMap, BTreeSet};
use std::marker::PhantomData;
use std::sync::Arc;

use futures_core::Future;
use util::replace_mut;

use super::step_inputs::StepInputs;
use super::sub_step::{SubStep, WrappedTransposer};
use crate::context::HandleInputContext;
// use super::interpolation::Interpolation;
// use super::lazy_state::LazyState;
// use super::step_metadata::{EmptyStepMetadata, StepMetadata};
// use super::sub_step::{PollErr as StepPollErr, SubStep, SubStepTime, WrappedTransposer};
use crate::schedule_storage::{DefaultStorage, StorageFamily};
// use crate::step::sub_step::SaturateErr;
use crate::{Transposer, TransposerInput, TransposerInputEventHandler};

pub struct Step<'almost_static, T: Transposer, Is: InputState<T>, S: StorageFamily = DefaultStorage>
where
    (T, Is): 'almost_static,
{
    inner:   StepInner<'almost_static, T, Is, S>,
    phantom: PhantomData<WrappedTransposer<T, S>>,

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
        // let mut steps = Vec::with_capacity(1);
        // let input_state = S::LazyState::new(Is::new());

        // steps.push(SubStep::new_init(transposer, rng_seed, &input_state));

        // Self {
        //     inner: StepInner::OriginalSaturating {
        //         current_saturating_index: 0,
        //         steps,
        //     },
        //     input_state,

        //     #[cfg(debug_assertions)]
        //     uuid_self: uuid::Uuid::new_v4(),
        //     #[cfg(debug_assertions)]
        //     uuid_prev: None,
        // }
        todo!()
    }

    pub fn next_unsaturated<I: TransposerInput<Base = T>>(
        &self,
        time: T::Time,
        inputs: &mut Option<StepInputs<T>>,
    ) -> Result<Self, NextUnsaturatedErr> {
        todo!()
    }

    pub fn saturate_take(&mut self, prev: &mut Self) -> Result<(), SaturateTakeErr> {
        #[cfg(debug_assertions)]
        if self.uuid_prev != Some(prev.uuid_self) {
            return Err(SaturateTakeErr::IncorrectPrevious)
        }
        todo!()
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

        todo!()
    }

    pub fn desaturate(&mut self) -> Result<(), DesaturateErr> {
        todo!()
    }

    pub fn poll(&mut self, waker: Waker) -> Result<StepPoll<T>, PollErr> {
        todo!()
    }

    // pub fn interpolate(&self, time: T::Time) -> Result<Interpolation<T, S>, InterpolateErr> {
    //     todo!()
    // }

    pub fn get_input_state(&self) -> &Is {
        todo!()
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

#[derive(Debug, PartialEq, Eq)]
pub enum StepPoll<T: Transposer> {
    NeedsState,
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
