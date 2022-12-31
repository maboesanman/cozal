use core::pin::Pin;
use core::task::{Context, Waker};
use std::marker::PhantomData;
use std::sync::Arc;

use util::replace_mut;

use super::sub_step::WrappedTransposer;
// use super::interpolation::Interpolation;
// use super::lazy_state::LazyState;
// use super::step_metadata::{EmptyStepMetadata, StepMetadata};
// use super::sub_step::{PollErr as StepPollErr, SubStep, SubStepTime, WrappedTransposer};
use crate::schedule_storage::{DefaultStorage, StorageFamily};
// use crate::step::sub_step::SaturateErr;
use crate::{Transposer, TransposerInput};

pub struct Step<T: Transposer, Is: InputState<T>, S: StorageFamily = DefaultStorage>
{
    // inner: StepInner<T, S>,
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
pub trait InputState<T: Transposer>
{
    fn new() -> Self;
    fn get_provider(&self) -> &T::InputStateManager;
}

impl<T: Transposer, Is: InputState<T>, S: StorageFamily> Step<T, Is, S>
{
    pub fn new_init(transposer: T, rng_seed: [u8; 32]) -> UnsaturatedStepBuilder<T, S> {
        // let mut steps = Vec::with_capacity(1);
        // let input_state = LazyState::new();

        // steps.push(SubStep::new_init(transposer, rng_seed, &input_state));

        todo!()

        // Self {
        //     phantom:     PhantomData,
        //     input_state: S::LazyState::new(Is::new()),

        //     // inner: StepInner::OriginalSaturating {
        //     //     current_saturating_index: 0,
        //     //     steps,
        //     //     metadata: M::new_init(),
        //     // },
        //     // input_state,
        //     #[cfg(debug_assertions)]
        //     uuid_self:                          uuid::Uuid::new_v4(),
        //     #[cfg(debug_assertions)]
        //     uuid_prev:                          None,
        // }
    }

    pub fn next_scheduled_unsaturated(&self) -> Result<Option<Self>, NextUnsaturatedErr> {
        todo!()
    }

    pub fn next_unsaturated<I: TransposerInput<Base = T>>(
        &self,
        time: T::Time,
        inputs: Box<[I::InputEvent]>,
    ) -> Result<NextUnsaturated<T, Is, S>, NextUnsaturatedErr> {
        todo!()
    }

    pub fn into_builder(self) -> UnsaturatedStepBuilder<T, S> {
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

pub struct UnsaturatedStepBuilder<T: Transposer, S: StorageFamily> {
    // inner: StepInner<T, S>,
    phantom: PhantomData<WrappedTransposer<T, S>>,
}

impl<T: Transposer, S: StorageFamily> UnsaturatedStepBuilder<T, S> {
    pub fn add_inputs<I: TransposerInput<Base = T>>(&mut self, inputs: Box<[I::InputEvent]>) {
        todo!()
    }

    pub fn complete<Is: InputState<T>>(self) -> Step<T, Is, S> {
        todo!()
    }
}

pub enum NextUnsaturated<T: Transposer, Is: InputState<T>, S: StorageFamily>
{
    Scheduled(Step<T, Is, S>),
    Input(UnsaturatedStepBuilder<T, S>),
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
