use std::collections::HashMap;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use futures_core::Future;

use super::pointer_interpolation::PointerInterpolation;
use super::step::{Step, StepTime, WrappedTransposer};
use super::step_group::{InterpolatePoll, InterpolatePollErr, NextUnsaturatedErr};
use super::NextInputs;
use crate::transposer::step_group::lazy_state::LazyState;
use crate::transposer::Transposer;

pub struct StepGroupSaturated<T: Transposer> {
    // first because it has pointers into boxes owned by steps.
    // not phantom pinned because steps are in a box, so don't care about moves.
    interpolations: HashMap<usize, Pin<Box<PointerInterpolation<T>>>>,
    steps:          Box<[Step<T>]>,
}

impl<T: Transposer> StepGroupSaturated<T> {
    pub fn next_unsaturated(
        &mut self,
        next_inputs: &mut NextInputs<T>,
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

                #[cfg(debug_assertions)]
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

    pub fn first_time(&self) -> &StepTime<T::Time> {
        self.steps.first().unwrap().time()
    }

    pub fn final_step(&self) -> &Step<T> {
        self.steps.last().unwrap()
    }

    // this lives as long as this stepgroup is alive
    fn final_wrapped_transposer(&self) -> &WrappedTransposer<T> {
        let final_step = self.final_step();
        final_step.finished_wrapped_transposer().unwrap()
    }

    pub fn poll_interpolate(
        &mut self,
        time: T::Time,
        channel: usize,
        waker: Waker,
    ) -> Result<InterpolatePoll<T>, InterpolatePollErr> {
        #[cfg(debug_assertions)]
        if time < self.first_time().raw_time() {
            return Err(InterpolatePollErr::TimePast)
        }

        // get current interpolation for channel.
        let mut current = self.interpolations.get_mut(&channel);

        // ignore current interpolation on channel if time doesn't match up.
        if let Some(int) = &current {
            if int.time() != time {
                current = None
            }
        }

        // use current, or insert, then use.
        let current = match current {
            // outdated
            Some(current) => current,
            // not present
            None => {
                let interpolation =
                    PointerInterpolation::new(time, self.final_wrapped_transposer());
                let interpolation = Box::pin(interpolation);
                self.interpolations.insert(channel, interpolation);
                self.interpolations.get_mut(&channel).unwrap()
            },
        };

        // poll and pass back results.
        let mut cx = Context::from_waker(&waker);
        match current.as_mut().poll(&mut cx) {
            Poll::Ready(out) => {
                self.interpolations.remove(&channel);
                Ok(InterpolatePoll::Ready(out))
            },
            Poll::Pending => {
                if current.needs_state() {
                    Ok(InterpolatePoll::NeedsState)
                } else {
                    Ok(InterpolatePoll::Pending)
                }
            },
        }
    }

    pub fn set_interpolation_input_state(
        &mut self,
        time: T::Time,
        channel: usize,
        state: T::InputState,
        ignore_waker: &Waker,
    ) -> Result<(), Box<T::InputState>> {
        match self.interpolations.get(&channel) {
            Some(int) => {
                if int.time() != time {
                    Err(Box::new(state))
                } else {
                    int.set_state(state, ignore_waker)
                }
            },
            None => Err(Box::new(state)),
        }
    }

    pub fn remove_interpolation(&mut self, channel: usize) {
        self.interpolations.remove(&channel);
    }
}

impl<T: Transposer> Drop for StepGroupSaturated<T> {
    fn drop(&mut self) {
        for i in self.interpolations.values_mut() {
            i.wake();
        }
    }
}
