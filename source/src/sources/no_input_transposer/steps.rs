use std::collections::{BTreeSet, VecDeque};

use transposer::context::{HandleScheduleContext, InitContext, InterpolateContext};
use transposer::step::{NoInput, NoInputManager, Step};
use transposer::Transposer;
use util::dummy_waker::DummyWaker;

pub struct Steps<T: Transposer<InputStateManager = NoInputManager>> {
    steps:                 VecDeque<StepWrapper<T>>,
    not_unsaturated:       BTreeSet<usize>,
    num_deleted_steps:     usize,
    deleted_before:        Option<T::Time>,
    number_of_checkpoints: usize,
}

impl<T: Transposer<InputStateManager = NoInputManager>> Steps<T> {
    pub fn new(transposer: T, start_time: T::Time, rng_seed: [u8; 32]) -> Self {
        let mut steps = VecDeque::new();
        let mut not_unsaturated = BTreeSet::new();

        steps.push_back(StepWrapper::new_init(transposer, start_time, rng_seed));
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

    fn saturate(&mut self, step_to_saturate: usize, pinned_times: &[T::Time]) {
        // println!("{:?}", step_to_saturate);

        let to_desaturate = self.sequence_number_to_delete(pinned_times, step_to_saturate);

        let take = match to_desaturate {
            Some(to_desaturate) => {
                let adjacent = to_desaturate == step_to_saturate - 1;
                if !adjacent {
                    self.desaturate(to_desaturate - self.num_deleted_steps);
                }

                adjacent
            },
            None => false,
        };

        self.saturate_adjacent(step_to_saturate - self.num_deleted_steps, take);
    }

    fn saturate_adjacent(&mut self, vecdeque_index: usize, take: bool) {
        let (previous, step) = self.get_step_and_prev_mut(vecdeque_index);

        if take {
            step.saturate_take(previous).unwrap();

            self.not_unsaturated
                .remove(&(vecdeque_index + self.num_deleted_steps - 1));
        } else {
            step.saturate_clone(previous).unwrap();
        }

        self.not_unsaturated
            .insert(vecdeque_index + self.num_deleted_steps);
    }

    fn desaturate(&mut self, vecdeque_index: usize) {
        self.steps
            .get_mut(vecdeque_index)
            .unwrap()
            .step
            .desaturate();
        self.not_unsaturated
            .remove(&(vecdeque_index + self.num_deleted_steps));
    }

    fn get_not_unsaturated_before_or_at_time(&self, time: T::Time) -> usize {
        let step_before = match self
            .steps
            .binary_search_by_key(&time, |s| s.step.get_time())
        {
            Ok(i) => i,
            Err(i) => i - 1,
        };

        let before_or_at_time = self.not_unsaturated.range(..=step_before);

        *before_or_at_time.last().unwrap()
    }

    fn sequence_number_to_delete(
        &self,
        pinned_times: &[T::Time],
        newly_saturated: usize,
    ) -> Option<usize> {
        // this cannot be:
        // - the earliest not-unsaturated step
        // - the latest not-unsaturated step (either the second to last step if its unsaturated, or else the last step)
        // - the latest not-unsaturated step before or at each pinned time
        // or else the "make progress" gurantees of the source will not be upheld.
        //
        // of the remaining, the minimum i by:
        // 1: the number of trailing zeroes of i
        // 2: i
        //
        // the effect is we try to preserve equally spaced checkpoints, so we don't have to recalculate too much.

        if self.number_of_checkpoints > self.not_unsaturated.len() {
            return None
        }

        let mut needed_steps = Vec::new();
        needed_steps.push(*self.not_unsaturated.first().unwrap());

        let last = *self.not_unsaturated.last().unwrap();
        if last + 1 != newly_saturated {
            needed_steps.push(*self.not_unsaturated.last().unwrap());
        }

        pinned_times
            .iter()
            .map(|t| self.get_not_unsaturated_before_or_at_time(*t))
            .collect_into(&mut needed_steps);

        needed_steps.sort();
        needed_steps.dedup();

        let candidates = self
            .not_unsaturated
            .iter()
            .map(|i| *i)
            .filter(|i| needed_steps.binary_search(i).is_err());

        candidates.min_by_key(|i| (i.trailing_zeros(), *i))
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

    fn try_next_scheduled(&mut self) {
        // ensure theres always a saturating or unsaturated step after the time polled.
        let last_step = &self.steps.back().unwrap().step;
        if last_step.is_saturated() {
            if let Some(new_step) = last_step.next_scheduled_unsaturated().unwrap() {
                self.steps.push_back(StepWrapper {
                    step: new_step
                })
            }
        }
    }

    pub fn get_before_or_at_events(
        &mut self,
        time: T::Time,
        pinned_times: &[T::Time],
    ) -> Result<BeforeStatusEvents<'_, T>, ()> {
        self.try_next_scheduled();

        let last_step_index = self.max_sequence_number();
        let last_step = &mut self.steps.back_mut().unwrap().step;

        if time < last_step.get_time() || !last_step.can_produce_events() {
            let next_time = (last_step.can_produce_events()).then_some(last_step.get_time());

            Ok(BeforeStatusEvents::Ready {
                next_time,
            })
        } else {
            if last_step.is_unsaturated() {
                self.saturate(last_step_index, pinned_times);
            }
            let last_step = &mut self.steps.back_mut().unwrap().step;
            Ok(BeforeStatusEvents::Saturating {
                step:       last_step,
                step_index: last_step_index,
            })
        }
    }

    pub fn get_before_or_at(
        &mut self,
        time: T::Time,
        pinned_times: &[T::Time],
    ) -> Result<BeforeStatus<'_, T>, ()> {
        Ok(
            match self.get_before_or_at_internal(time, pinned_times, false)? {
                BeforeStatusInternal::SaturatedReady(i) => {
                    let original_step = self.steps.back().unwrap();
                    let next_time = (!original_step.step.is_saturated())
                        .then_some(original_step.step.get_time());

                    BeforeStatus::Saturated {
                        step: &mut self.steps.get_mut(i).unwrap().step,
                        next_time,
                    }
                },
                BeforeStatusInternal::Saturating(i) => {
                    let step_index = i + self.num_deleted_steps;
                    let step = &mut self.steps.get_mut(i).unwrap().step;

                    BeforeStatus::Saturating {
                        step,
                        step_index,
                    }
                },
                _ => panic!(),
            },
        )
    }

    fn get_before_or_at_internal(
        &mut self,
        time: T::Time,
        pinned_times: &[T::Time],
        events_only: bool,
    ) -> Result<BeforeStatusInternal, ()> {
        self.try_next_scheduled();

        if events_only {
            let last_step = &self.steps.back_mut().unwrap().step;
            if time < last_step.get_time() || !last_step.can_produce_events() {
                return Ok(BeforeStatusInternal::AllRepeat)
            } else {
                return Ok(BeforeStatusInternal::Saturating(self.max_sequence_number()))
            }
        }

        // this is just mimicking partition_point, because vecdeque isn't actually contiguous
        let vecdeque_index = match self
            .steps
            .binary_search_by_key(&time, |s| s.step.get_time())
        {
            Ok(i) => i,
            Err(i) => i.checked_sub(1).ok_or(())?,
        };

        // now i is the index of the latest step before or at time.

        let step = self.steps.get_mut(vecdeque_index).ok_or(())?;
        if step.step.is_saturated() {
            return Ok(BeforeStatusInternal::SaturatedReady(vecdeque_index))
        } else if step.step.is_saturating() {
            return Ok(BeforeStatusInternal::Saturating(vecdeque_index))
        }

        loop {
            let vecdeque_index = vecdeque_index.checked_sub(1).ok_or(())?;
            let step = self.steps.get_mut(vecdeque_index).ok_or(())?;

            if step.step.is_saturated() {
                let vecdeque_index = vecdeque_index + 1;
                let step_to_saturate = vecdeque_index + self.num_deleted_steps;
                self.saturate(step_to_saturate, pinned_times);
                return Ok(BeforeStatusInternal::Saturating(vecdeque_index))
            }

            if step.step.is_saturating() {
                return Ok(BeforeStatusInternal::Saturating(vecdeque_index))
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
    },
}

pub enum BeforeStatusEvents<'a, T: Transposer<InputStateManager = NoInputManager>> {
    Ready {
        next_time: Option<T::Time>,
    },
    Saturating {
        step:       &'a mut Step<T, NoInput>,
        step_index: usize,
    },
}

pub enum BeforeStatusInternal {
    SaturatedReady(usize),
    Saturating(usize),
    AllRepeat,
}

struct StepWrapper<T: Transposer<InputStateManager = NoInputManager>> {
    pub step: Step<T, NoInput>,
}

impl<T: Transposer<InputStateManager = NoInputManager>> StepWrapper<T> {
    pub fn new_init(transposer: T, start_time: T::Time, rng_seed: [u8; 32]) -> Self {
        Self {
            step: Step::new_init(transposer, start_time, rng_seed),
        }
    }
}

#[derive(Clone)]
pub(crate) struct CollatzTransposer {
    value: usize,
}

impl CollatzTransposer {
    pub fn new(value: usize) -> Self {
        Self {
            value,
        }
    }
}

impl Transposer for CollatzTransposer {
    type Time = usize;

    type OutputState = ();

    type Scheduled = ();

    type OutputEvent = usize;

    // set up with macro
    type InputStateManager = NoInputManager;

    async fn init(&mut self, cx: &mut dyn InitContext<'_, Self>) {
        cx.schedule_event(cx.current_time(), ()).unwrap();
    }

    async fn handle_scheduled(
        &mut self,
        _payload: Self::Scheduled,
        cx: &mut dyn HandleScheduleContext<'_, Self>,
    ) {
        cx.emit_event(self.value).await;

        if self.value % 2 == 0 {
            self.value = self.value / 2;
        } else {
            self.value = self.value * 3 + 1;
        }

        cx.schedule_event(cx.current_time() + 1, ()).unwrap();
    }

    async fn interpolate(&self, _cx: &mut dyn InterpolateContext<'_, Self>) -> Self::OutputState {}
}

#[test]
fn basic_test() {
    let transposer = CollatzTransposer::new(27);
    let mut steps = Steps::new(transposer, 0, [0; 32]);
    let dummy = DummyWaker::dummy();
    for _ in 0..200 {
        let _ = match steps.get_before_or_at_events(100000, &[]).unwrap() {
            BeforeStatusEvents::Ready {
                ..
            } => panic!(),
            BeforeStatusEvents::Saturating {
                step, ..
            } => step.poll(&dummy).unwrap(),
        };
    }

    let output = format!("{:?}", steps.not_unsaturated);
    assert_eq!(output, "{0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 60, 64, 68, 72, 76, 80, 84, 86, 88, 90, 92, 94, 96, 98, 99}");

    steps.delete_before(30);

    let output = format!("{:?}", steps.not_unsaturated);
    assert_eq!(
        output,
        "{60, 64, 68, 72, 76, 80, 84, 86, 88, 90, 92, 94, 96, 98, 99}"
    );

    for _ in 0..200 {
        let _ = match steps.get_before_or_at_events(100000, &[]).unwrap() {
            BeforeStatusEvents::Ready {
                ..
            } => panic!(),
            BeforeStatusEvents::Saturating {
                step, ..
            } => step.poll(&dummy).unwrap(),
        };
    }

    let output = format!("{:?}", steps.not_unsaturated);
    assert_eq!( output, "{60, 64, 72, 80, 88, 96, 104, 112, 116, 120, 124, 128, 132, 136, 140, 144, 148, 152, 156, 160, 164, 168, 172, 176, 180, 184, 188, 192, 196, 199}");
}
