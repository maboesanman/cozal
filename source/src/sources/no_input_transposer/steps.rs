use std::collections::{BTreeSet, VecDeque};

use transposer::step::{NoInput, NoInputManager, Step};
use transposer::Transposer;

pub struct Steps<T: Transposer<InputStateManager = NoInputManager>> {
    steps:                 VecDeque<StepWrapper<T>>,
    not_unsaturated:       BTreeSet<usize>,
    num_deleted_steps:     usize,
    deleted_before:        Option<T::Time>,
    number_of_checkpoints: usize,
}

impl<T: Transposer<InputStateManager = NoInputManager>> Steps<T> {
    pub fn new(transposer: T, rng_seed: [u8; 32]) -> Self {
        let mut steps = VecDeque::new();
        let mut not_unsaturated = BTreeSet::new();

        steps.push_back(StepWrapper::new_init(transposer, rng_seed));
        not_unsaturated.insert(0);

        Self {
            steps,
            not_unsaturated,
            num_deleted_steps: 0,
            deleted_before: None,
            number_of_checkpoints: 30,
        }
    }

    pub fn get_by_sequence_number(&self, i: usize) -> Option<&Step<T, NoInput>> {
        let i = i.checked_sub(self.num_deleted_steps)?;

        self.steps.get(i).map(|s| &s.step)
    }

    pub fn get_mut_by_sequence_number(&mut self, i: usize) -> Option<&mut Step<T, NoInput>> {
        let i = i.checked_sub(self.num_deleted_steps)?;

        self.steps.get_mut(i).map(|s| &mut s.step)
    }

    pub fn get_last(&self) -> &Step<T, NoInput> {
        &self.steps.back().unwrap().step
    }

    pub fn get_last_mut(&mut self) -> &mut Step<T, NoInput> {
        &mut self.steps.back_mut().unwrap().step
    }

    pub fn get_scheduled_time(&self) -> Option<T::Time> {
        let step = self.get_last();

        (!step.is_saturated()).then_some(step.get_time())
    }

    fn max_sequence_number(&self) -> usize {
        self.num_deleted_steps + self.steps.len() - 1
    }

    fn saturate(&mut self, /* index in vecdeq currently */ i: usize, take: bool) {
        let (previous, step) = self.get_step_and_prev_mut(i);

        if take {
            step.saturate_take(previous).unwrap();

            self.not_unsaturated
                .remove(&(i + self.num_deleted_steps - 1));
        } else {
            step.saturate_clone(previous).unwrap();
        }

        self.not_unsaturated.insert(i + self.num_deleted_steps);
    }

    fn desaturate(&mut self, /* index in vecdeq currently */ i: usize) {
        self.steps.get_mut(i).unwrap().step.desaturate();
        self.not_unsaturated.remove(&(i + self.num_deleted_steps));
    }

    fn sequence_number_to_delete(
        &mut self,
        pinned_times: &[T::Time],
        newly_saturated: usize,
    ) -> Option<usize> {
        if self.number_of_checkpoints > self.not_unsaturated.len() {
            return None
        }

        if let Some(step) = self.get_by_sequence_number(newly_saturated) {
            if let Some(deleted_before) = self.deleted_before {
                if step.get_time() <= deleted_before {
                    return Some(newly_saturated - 1)
                }
            }
        }

        let mut candidates: Vec<_> = self.not_unsaturated.iter().map(|x| *x).collect();

        let mut pinned_steps = vec![self.num_deleted_steps];

        pinned_times
            .iter()
            .map(|t| {
                match candidates.binary_search_by_key(t, |c| {
                    self.get_by_sequence_number(*c).unwrap().get_time()
                }) {
                    Ok(i) => i,
                    Err(i) => i - 1,
                }
            })
            .collect_into(&mut pinned_steps);

        // pinned_steps.sort();
        pinned_steps.dedup();

        for pinned in pinned_steps.iter().rev() {
            candidates.remove(*pinned);
        }

        candidates
            .iter()
            .min_by_key(|i| i.trailing_zeros())
            .map(|x| *x)
    }

    fn get_step_and_prev_mut(
        &mut self,
        i: usize,
    ) -> (&mut Step<T, NoInput>, &mut Step<T, NoInput>) {
        // this is all a dance to get a mutable reference to
        // steps[i] and steps[i - 1] simultaneously with no unsafe
        let (front, back) = self.steps.as_mut_slices();

        let (before, after) = match i.cmp(&front.len()) {
            std::cmp::Ordering::Less => front.split_at_mut(i),
            std::cmp::Ordering::Equal => (front, back),
            std::cmp::Ordering::Greater => back.split_at_mut(i - front.len()),
        };

        (
            &mut before.last_mut().unwrap().step,
            &mut after.first_mut().unwrap().step,
        )
    }

    pub fn get_before_or_at(
        &mut self,
        time: T::Time,
        pinned_times: &[T::Time],
    ) -> Result<BeforeStatus<'_, T>, ()> {
        Ok(match self.get_before_or_at_internal(time, pinned_times)? {
            (BeforeStatusInternal::SaturatedReady, i) => {
                let original_step = self.steps.back().unwrap();
                let next_time =
                    (!original_step.step.is_saturated()).then_some(original_step.step.get_time());

                BeforeStatus::Saturated {
                    step: &mut self.steps.get_mut(i).unwrap().step,
                    next_time,
                }
            },
            (BeforeStatusInternal::Saturating, i) => {
                let repeat = i + 1 == self.steps.len();
                let step_index = i + self.num_deleted_steps;
                let step = &mut self.steps.get_mut(i).unwrap().step;

                BeforeStatus::Saturating {
                    step,
                    step_index,
                    repeat,
                }
            },
        })
    }

    fn get_before_or_at_internal(
        &mut self,
        time: T::Time,
        pinned_times: &[T::Time],
    ) -> Result<(BeforeStatusInternal, usize), ()> {
        // ensure theres always a saturating or unsaturated step after the time polled.
        let last_step = &self.steps.back().unwrap().step;
        if time > last_step.get_time() && last_step.is_saturated() {
            if let Some(new_step) = last_step.next_scheduled_unsaturated().unwrap() {
                self.steps.push_back(StepWrapper {
                    step:             new_step,
                    first_emitted_id: None,
                })
            }
        }

        // this is just mimicking partition_point, because vecdeque isn't actually contiguous
        let mut i = match self
            .steps
            .binary_search_by_key(&time, |s| s.step.get_time())
        {
            Ok(i) => i,
            Err(i) => i.checked_sub(1).ok_or(())?,
        };

        // now i is the index of the latest step before or at time.

        // this is only indexed into in two places. here and in the loop.
        let step_i = self.steps.get_mut(i).ok_or(())?;
        if step_i.step.is_saturated() {
            return Ok((BeforeStatusInternal::SaturatedReady, i))
        } else if step_i.step.is_saturating() {
            return Ok((BeforeStatusInternal::Saturating, i))
        }

        loop {
            i = i.checked_sub(1).ok_or(())?;

            let step_i = self.steps.get_mut(i).ok_or(())?;

            if step_i.step.is_saturated() {
                let to_desaturate =
                    self.sequence_number_to_delete(pinned_times, i + self.num_deleted_steps);

                let take = match to_desaturate {
                    Some(to_desaturate) => {
                        let adjacent = to_desaturate == i + self.num_deleted_steps - 1;
                        if !adjacent {
                            self.desaturate(to_desaturate);
                        }

                        adjacent
                    },
                    None => false,
                };

                self.saturate(i + 1, take);

                return Ok((BeforeStatusInternal::Saturating, i + 1))
            } else if step_i.step.is_saturating() {
                return Ok((BeforeStatusInternal::Saturating, i))
            }
        }
    }

    pub fn delete_before(&mut self, time: T::Time) {
        if let Some(t) = self.deleted_before {
            if time <= t {
                return
            }
        }

        self.deleted_before = Some(time);

        // this is just mimicking partition_point, because vecdeque isn't actually contiguous
        let i = match self
            .steps
            .binary_search_by_key(&time, |s| s.step.get_time())
        {
            Ok(i) => i,
            Err(i) => match i.checked_sub(1) {
                Some(i) => i,
                None => return,
            },
        };

        // now i is the index of the latest step we need to keep.
        let to_retain = self.steps.split_off(i);
        self.num_deleted_steps += self.steps.len();
        self.steps = to_retain;

        let sequence_number = i + self.num_deleted_steps;
        let to_retain = self.not_unsaturated.split_off(&sequence_number);
        self.not_unsaturated = to_retain;
    }
}

pub enum BeforeStatus<'a, T: Transposer<InputStateManager = NoInputManager>> {
    Saturated {
        step:      &'a Step<T, NoInput>,
        next_time: Option<T::Time>,
    },
    Saturating {
        step:       &'a mut Step<T, NoInput>,
        step_index: usize,
        repeat:     bool,
    },
}

pub enum BeforeStatusInternal {
    SaturatedReady,
    Saturating,
}

struct StepWrapper<T: Transposer<InputStateManager = NoInputManager>> {
    pub step:             Step<T, NoInput>,
    pub first_emitted_id: Option<usize>,
}

impl<T: Transposer<InputStateManager = NoInputManager>> StepWrapper<T> {
    pub fn new_init(transposer: T, rng_seed: [u8; 32]) -> Self {
        Self {
            step:             Step::new_init(transposer, rng_seed),
            first_emitted_id: None,
        }
    }
}
