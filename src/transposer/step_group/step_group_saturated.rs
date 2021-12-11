use std::collections::HashMap;
use std::task::Waker;

use super::pointer_interpolation::PointerInterpolation;
use super::step::{Step, StepTime};
use super::step_group::NextUnsaturatedErr;
use crate::transposer::step_group::lazy_state::LazyState;
use crate::transposer::step_group::step_group::StepGroup;
use crate::transposer::Transposer;

pub struct StepGroupSaturated<T: Transposer> {
    // first because it has pointers into boxes owned by steps.
    // not phantom pinned because steps are in a box, so don't care about moves.
    interpolations: HashMap<usize, PointerInterpolation<T>>,
    steps:          Box<[Step<T>]>,
}

impl<T: Transposer> StepGroupSaturated<T> {
    pub fn next_unsaturated(
        &mut self,
        next_inputs: &mut Option<(T::Time, Box<[T::Input]>)>,
        input_state: *const LazyState<T::InputState>,
    ) -> Result<Option<Step<T>>, NextUnsaturatedErr> {
        // drain interpolations from self that occur after input
        let next_time: Option<T::Time> = next_inputs.as_ref().map(|(t, _)| *t);
        if let Some(next_time) = next_time {
            for (_, interpolation) in self
                .interpolations
                .drain_filter(|_, v| v.time() >= next_time)
            {
                interpolation.wake();
            }
        }

        let next_step = self
            .final_step()
            .next_unsaturated(next_inputs, input_state)
            .map_err(|e| match e {
                super::step::NextUnsaturatedErr::NotSaturated => unreachable!(),
                super::step::NextUnsaturatedErr::InputPastOrPresent => {
                    NextUnsaturatedErr::InputPastOrPresent
                },
            })?;

        Ok(next_step)
    }

    // last step is always saturated.
    pub fn take(mut self) -> Result<Box<[Step<T>]>, Self> {
        if self.interpolations.is_empty() {
            Ok(core::mem::take(&mut self.steps))
        } else {
            Err(self)
        }
    }

    pub fn new(steps: Box<[Step<T>]>) -> Self {
        Self {
            steps,
            interpolations: HashMap::new(),
        }
    }

    pub fn time(&self) -> &StepTime<T::Time> {
        self.steps.first().unwrap().time()
    }

    pub fn final_step(&self) -> &Step<T> {
        self.steps.last().unwrap()
    }

    // this lives as long as this stepgroup is alive
    fn final_transposer(&self) -> &T {
        let final_step = self.final_step();
        &final_step.finished_wrapped_transposer().unwrap().transposer
    }

    #[allow(dead_code)]
    pub fn poll_interpolate(
        &mut self,
        time: T::Time,
        channel: usize,
        waker: Waker,
    ) -> Result<(), InterpolatePollErr> {
        let base_time = self.time().raw_time();
        #[cfg(debug_assertions)]
        if time < base_time {
            return Err(InterpolatePollErr::TimePast)
        }

        // check if we already have an interpolation for this channel
        // if the time matches we can keep going with this.

        let base = self.final_step().finished_wrapped_transposer().unwrap();
        let fut = T::interpolate(&base.transposer, base_time, time, todo!());
        // interpolations.insert(channel, v)

        todo!()
    }

    #[allow(dead_code)]
    pub fn remove_interpolation(&mut self, channel: usize) {
        self.interpolations.remove(&channel);
    }
}

impl<T: Transposer> Drop for StepGroupSaturated<T> {
    fn drop(&mut self) {
        for (_, i) in &mut self.interpolations {
            i.wake();
        }
    }
}

pub enum InterpolatePollErr {
    NotSaturated,
    #[cfg(debug_assertions)]
    TimePast,
}
