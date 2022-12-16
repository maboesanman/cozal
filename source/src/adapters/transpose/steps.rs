use std::collections::VecDeque;
use std::ops::{Range, RangeInclusive};
use std::ptr::NonNull;
use std::slice::SliceIndex;
use std::task::Poll;

use transposer::schedule_storage::StorageFamily;
// use super::transpose_step_metadata::TransposeStepMetadata;
use transposer::step::{Interpolation, Step};
use transposer::Transposer;
use util::extended_entry::vecdeque::{get_ext_entry, ExtEntry};
use util::vecdeque_helpers::get_with_next_mut;

use super::input_buffer::{InputBuffer, InputBufferEntry};
use super::storage::{DummySendStorage, TransposeStorage};

// a collection of Rc which are guranteed not to be cloned outside the collection is Send
// whenever the same collection, but with Arc would be Send, so we do an unsafe impl for exactly that situation.
unsafe impl<T: Transposer> Send for Steps<T> where Step<T, DummySendStorage>: Send {}

pub struct Steps<T: Transposer> {
    steps:             VecDeque<StepWrapper<T>>,
    num_deleted_steps: usize,
}

impl<T: Transposer> Steps<T> {
    pub fn new(transposer: T, rng_seed: [u8; 32]) -> Self {
        let mut steps = VecDeque::new();
        steps.push_back(StepWrapper::new_init(transposer, rng_seed));
        Self {
            steps,
            num_deleted_steps: 0,
        }
    }

    pub fn poll<S: StorageFamily>(
        &mut self,
        time: T::Time,
        input_buffer: &mut InputBuffer<T>,
    ) -> StepsPoll<T, S> {
        todo!()
    }

    pub fn try_poll_shared<S: StorageFamily>(&self, time: T::Time) -> Option<Interpolation<T, S>> {
        todo!()
    }

    pub fn rollback(
        &mut self,
        time: T::Time,
        input_buffer: &mut InputBuffer<T>,
    ) -> StepsRollback<T> {
        todo!()
    }

    pub fn finalize(&mut self, time: T::Time) {
        todo!()
    }

    pub fn advance(&mut self, time: T::Time) {
        todo!()
    }

    fn get_by_sequence_number(&self, i: usize) -> Option<&StepWrapper<T>> {
        let i = i.checked_sub(self.num_deleted_steps)?;

        self.steps.get(i)
    }

    fn get_mut_by_sequence_number(&mut self, i: usize) -> Option<&mut StepWrapper<T>> {
        let i = i.checked_sub(self.num_deleted_steps)?;

        self.steps.get_mut(i)
    }

    fn get_last(&self) -> &StepWrapper<T> {
        self.steps.back().unwrap()
    }

    fn get_last_mut(&mut self) -> &mut StepWrapper<T> {
        self.steps.back_mut().unwrap()
    }

    fn get_before_or_at(&mut self, time: T::Time) -> Result<BeforeStatus<'_, T>, ()> {
        // this is just mimicking partition_point, because vecdeque isn't actually contiguous
        let mut i = match self
            .steps
            .binary_search_by_key(&time, |s| s.step.raw_time())
        {
            Ok(i) => i,
            Err(i) => i.checked_sub(1).ok_or(())?,
        };

        // this is only indexed into in two places. here and in the loop.
        let steps = unsafe { Into::<std::ptr::NonNull<_>>::into(&mut self.steps).as_mut() };
        let mut step_i = steps.get_mut(i).ok_or(())?;
        if step_i.step.is_saturated() {
            // SAFETY: This line can be deleted with polonius
            let step_i = unsafe { Into::<std::ptr::NonNull<_>>::into(step_i).as_ref() };
            return Ok(BeforeStatus::SaturatedImmediate(step_i))
        }

        let mut step_next;

        i = i.checked_sub(1).ok_or(())?;
        while i > 0 {
            step_next = step_i;
            // this is only indexed into in two places. here and at the declaration of step_i.
            let steps = unsafe { Into::<std::ptr::NonNull<_>>::into(&mut self.steps).as_mut() };
            step_i = steps.get_mut(i).ok_or(())?;
            if step_i.step.is_unsaturated() {
                i -= 1;
                continue
            }

            if step_i.step.is_saturating() {
                // SAFETY: This line can be deleted with polonius
                let step_i = unsafe { Into::<std::ptr::NonNull<_>>::into(step_i).as_mut() };
                return Ok(BeforeStatus::Saturating(step_i))
            }

            // SAFETY: This line can be deleted with polonius
            let step_i = unsafe { Into::<std::ptr::NonNull<_>>::into(step_i).as_mut() };
            return Ok(BeforeStatus::SaturatedDistant(step_i, step_next))
        }

        Err(())
    }

    pub fn delete_before(&mut self, time: T::Time) {}
}

pub struct StepsPoll<T: Transposer, S: StorageFamily> {
    completed_steps: Option<RangeInclusive<usize>>,
    result:          StepsPollResult<T, S>,
}

pub enum StepsPollResult<T: Transposer, S: StorageFamily> {
    Ready(Interpolation<T, S>),
    Pending(/* step_id */ usize),
    NeedsState(/* step_id */ usize),
    Event(/* step_id */ usize, T::Time, T::OutputEvent),
}

pub struct StepsRollback<T: Transposer> {
    rollback_steps: Option<Range</* step_id */ usize>>,
    rollback_time:  Option<T::Time>,
}

pub struct StepWrapper<T: Transposer> {
    pub step:             Step<T, TransposeStorage>,
    pub first_emitted_id: Option<usize>,
}

impl<T: Transposer> StepWrapper<T> {
    pub fn new_init(transposer: T, rng_seed: [u8; 32]) -> Self {
        Self {
            step:             Step::new_init(transposer, rng_seed),
            first_emitted_id: None,
        }
    }
}

pub enum BeforeStatus<'a, T: Transposer> {
    SaturatedImmediate(&'a StepWrapper<T>),
    SaturatedDistant(&'a mut StepWrapper<T>, &'a mut StepWrapper<T>),
    Saturating(&'a mut StepWrapper<T>),
}
