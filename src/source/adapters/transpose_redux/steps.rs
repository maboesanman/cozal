use std::collections::VecDeque;
use std::slice::SliceIndex;

use super::input_buffer::InputBuffer;
use super::storage::{DummySendStorage, TransposeStorage};
// use super::transpose_step_metadata::TransposeStepMetadata;
use crate::transposer::step::Step;
use crate::transposer::Transposer;
use crate::util::extended_entry::vecdeque::{get_ext_entry, ExtEntry};

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

    pub fn get_by_sequence_number(&self, i: usize) -> Option<&StepWrapper<T>> {
        let i = i.checked_sub(self.num_deleted_steps)?;

        self.steps.get(i)
    }
    pub fn get_mut_by_sequence_number(&mut self, i: usize) -> Option<&mut StepWrapper<T>> {
        let i = i.checked_sub(self.num_deleted_steps)?;

        self.steps.get_mut(i)
    }

    pub fn get_entry_by_sequence_number(
        &mut self,
        i: usize,
    ) -> Option<ExtEntry<'_, StepWrapper<T>>> {
        let i = i.checked_sub(self.num_deleted_steps)?;

        get_ext_entry(&mut self.steps, i)
    }

    pub fn get_last_entry(&mut self) -> Option<ExtEntry<'_, StepWrapper<T>>> {
        let i = self.steps.len().checked_sub(1)?;

        get_ext_entry(&mut self.steps, i)
    }

    pub fn get_last_mut(&mut self) -> &mut StepWrapper<T> {
        self.steps.back_mut().unwrap()
    }

    pub fn get_last_saturating_before_or_at(
        &mut self,
        time: T::Time,
    ) -> Option<&mut StepWrapper<T>> {
        let mut i = match self
            .steps
            .binary_search_by_key(&time, |s| s.step.raw_time())
        {
            Ok(i) => i,
            Err(i) => i.checked_sub(1)?,
        };

        while i > 0 {
            i -= 1;
            if !self.steps.get(i).unwrap().step.is_unsaturated() {
                return self.steps.get_mut(i)
            }
        }

        None
    }
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
