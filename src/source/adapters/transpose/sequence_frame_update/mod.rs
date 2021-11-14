use core::cmp::Ordering;
use core::mem::{replace, MaybeUninit};
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

use arg::{Arg, InitArg, InputArg, ScheduledArg};
use futures_core::Future;
use update_context::UpdateContext;
use update_result::UpdateResult;

use super::input_buffer::InputBuffer;
use crate::transposer::Transposer;

mod arg;
mod frame_update;
mod frame_update_pollable;
mod lazy_state;
#[cfg(test)]
mod test;
mod update_context;
mod update_result;
use self::engine_time::EngineTime;
use self::frame::Frame;
use self::frame_update::FrameUpdate;
use self::update_context::{OriginalUpdateContext, RepeatUpdateContext};
pub(self) use super::*;

pub struct SequenceFrameUpdate<T: Transposer> {
    time:            EngineTime<T::Time>,
    inner:           SequenceFrameUpdateInner<T>,
    outputs_emitted: OutputsEmitted,
}

impl<T: Transposer> SequenceFrameUpdate<T> {
    pub fn new_init(transposer: T, rng_seed: [u8; 32]) -> Self {
        let time = EngineTime::new_init();
        let frame = Frame::new(transposer, rng_seed);
        let frame = Box::new(frame);
        let update = FrameUpdate::new(frame, (), time.clone());
        let update = Box::pin(update);
        let inner = SequenceFrameUpdateInner::SaturatingInit {
            update,
        };
        SequenceFrameUpdate {
            time,
            inner,
            outputs_emitted: OutputsEmitted::Pending,
        }
    }

    pub fn next_unsaturated(
        &self,
        input_buffer: &mut InputBuffer<T::Time, T::Input>,
    ) -> Result<Option<Self>, ()> {
        let frame = match &self.inner {
            SequenceFrameUpdateInner::SaturatedInit {
                frame, ..
            } => frame.as_ref(),
            SequenceFrameUpdateInner::SaturatedInput {
                frame, ..
            } => frame.as_ref(),
            SequenceFrameUpdateInner::SaturatedScheduled {
                frame, ..
            } => frame.as_ref(),
            _ => return Err(()),
        };
        let next_input_time = input_buffer.first_time();
        let next_scheduled_time = frame.get_next_scheduled_time();

        let next_time_index = self.time.index() + 1;

        let (time, is_input) = match (next_input_time, next_scheduled_time) {
            (None, None) => return Ok(None),
            (None, Some(t)) => (EngineTime::new_scheduled(next_time_index, t.clone()), false),
            (Some(t), None) => (EngineTime::new_input(next_time_index, t), true),
            (Some(t_i), Some(t_s)) => match t_i.cmp(&t_s.time) {
                Ordering::Greater => (
                    EngineTime::new_scheduled(next_time_index, t_s.clone()),
                    false,
                ),
                _ => (EngineTime::new_input(next_time_index, t_i), true),
            },
        };

        let item = if is_input {
            SequenceFrameUpdate {
                time,
                inner: SequenceFrameUpdateInner::OriginalUnsaturatedInput {
                    inputs: input_buffer.pop_first().unwrap().1,
                },
                outputs_emitted: OutputsEmitted::Pending,
            }
        } else {
            SequenceFrameUpdate {
                time,
                inner: SequenceFrameUpdateInner::OriginalUnsaturatedScheduled,
                outputs_emitted: OutputsEmitted::Pending,
            }
        };

        Ok(Some(item))
    }

    // previous is expected to be the value produced this via next_unsaturated.
    pub fn saturate_take(&mut self, previous: &mut Self) -> Result<(), ()> {
        if !(previous.inner.is_saturated() && self.inner.is_unsaturated()) {
            return Err(())
        }

        let mut frame_dest = MaybeUninit::uninit();

        take_mut::take_or_recover(
            &mut previous.inner,
            || SequenceFrameUpdateInner::Unreachable,
            |prev| match prev {
                SequenceFrameUpdateInner::SaturatedInit {
                    frame,
                } => {
                    frame_dest = MaybeUninit::new(frame);
                    SequenceFrameUpdateInner::UnsaturatedInit
                },
                SequenceFrameUpdateInner::SaturatedInput {
                    inputs,
                    frame,
                } => {
                    frame_dest = MaybeUninit::new(frame);
                    SequenceFrameUpdateInner::RepeatUnsaturatedInput {
                        inputs,
                    }
                },
                SequenceFrameUpdateInner::SaturatedScheduled {
                    frame,
                } => {
                    frame_dest = MaybeUninit::new(frame);
                    SequenceFrameUpdateInner::RepeatUnsaturatedScheduled
                },
                _ => unreachable!(),
            },
        );

        let frame = unsafe { frame_dest.assume_init() };

        self.saturate_from_frame(frame)?;

        Ok(())
    }

    // previous is expected to be the value produced this via next_unsaturated.
    pub fn saturate_clone(&mut self, previous: &Self) -> Result<(), ()>
    where
        T: Clone,
    {
        let frame = match &previous.inner {
            SequenceFrameUpdateInner::SaturatedInit {
                frame, ..
            } => Ok(frame.clone()),
            SequenceFrameUpdateInner::SaturatedInput {
                frame, ..
            } => Ok(frame.clone()),
            SequenceFrameUpdateInner::SaturatedScheduled {
                frame,
            } => Ok(frame.clone()),
            _ => Err(()),
        }?;

        self.saturate_from_frame(frame)?;

        Ok(())
    }

    fn saturate_from_frame(&mut self, frame: Box<Frame<T>>) -> Result<(), ()> {
        if !self.inner.is_unsaturated() {
            return Err(())
        }

        take_mut::take_or_recover(
            &mut self.inner,
            || SequenceFrameUpdateInner::Unreachable,
            |next| match next {
                SequenceFrameUpdateInner::OriginalUnsaturatedInput {
                    inputs,
                } => {
                    let update = FrameUpdate::new(frame, inputs, self.time.clone());
                    SequenceFrameUpdateInner::OriginalSaturatingInput {
                        update: Box::pin(update),
                    }
                },
                SequenceFrameUpdateInner::RepeatUnsaturatedInput {
                    inputs,
                } => {
                    let update = FrameUpdate::new(frame, inputs, self.time.clone());
                    SequenceFrameUpdateInner::RepeatSaturatingInput {
                        update: Box::pin(update),
                    }
                },
                SequenceFrameUpdateInner::OriginalUnsaturatedScheduled => {
                    let update = FrameUpdate::new(frame, (), self.time.clone());
                    SequenceFrameUpdateInner::OriginalSaturatingScheduled {
                        update: Box::pin(update),
                    }
                },
                SequenceFrameUpdateInner::RepeatUnsaturatedScheduled => {
                    let update = FrameUpdate::new(frame, (), self.time.clone());
                    SequenceFrameUpdateInner::RepeatSaturatingScheduled {
                        update: Box::pin(update),
                    }
                },
                _ => unreachable!(),
            },
        );

        Ok(())
    }

    pub fn desaturate(&mut self) -> Result<(), ()> {
        let mut result = Err(());
        take_mut::take(&mut self.inner, |original| match original {
            SequenceFrameUpdateInner::OriginalSaturatingInput {
                mut update,
            } => {
                let inputs = update.as_mut().reclaim();
                if inputs.is_ok() {
                    result = Ok(())
                };
                SequenceFrameUpdateInner::OriginalUnsaturatedInput {
                    inputs: inputs.unwrap_or(Box::new([])),
                }
            },
            SequenceFrameUpdateInner::RepeatSaturatingInput {
                mut update,
            } => {
                let inputs = update.as_mut().reclaim();
                if inputs.is_ok() {
                    result = Ok(())
                };
                SequenceFrameUpdateInner::RepeatUnsaturatedInput {
                    inputs: inputs.unwrap_or(Box::new([])),
                }
            },
            SequenceFrameUpdateInner::OriginalSaturatingScheduled {
                update: _,
            } => {
                result = Ok(());
                SequenceFrameUpdateInner::OriginalUnsaturatedScheduled
            },
            SequenceFrameUpdateInner::RepeatSaturatingScheduled {
                update: _,
            } => {
                result = Ok(());
                SequenceFrameUpdateInner::RepeatUnsaturatedScheduled
            },
            SequenceFrameUpdateInner::SaturatedInit {
                ..
            } => {
                result = Ok(());
                SequenceFrameUpdateInner::UnsaturatedInit
            },
            SequenceFrameUpdateInner::SaturatedInput {
                inputs, ..
            } => {
                result = Ok(());
                SequenceFrameUpdateInner::RepeatUnsaturatedInput {
                    inputs,
                }
            },
            SequenceFrameUpdateInner::SaturatedScheduled {
                ..
            } => {
                result = Ok(());
                SequenceFrameUpdateInner::RepeatUnsaturatedScheduled
            },
            other => other,
        });

        result
    }

    pub fn rollback(
        mut self,
        input_buffer: &mut InputBuffer<T::Time, T::Input>,
    ) -> Result<bool, ()> {
        let time = self.time.raw_time();
        let inputs = match replace(&mut self.inner, SequenceFrameUpdateInner::Unreachable) {
            SequenceFrameUpdateInner::OriginalUnsaturatedInput {
                inputs,
            } => Some(inputs),
            SequenceFrameUpdateInner::RepeatUnsaturatedInput {
                inputs,
            } => Some(inputs),
            SequenceFrameUpdateInner::OriginalSaturatingInput {
                mut update,
            } => Some(update.as_mut().reclaim()?),
            SequenceFrameUpdateInner::RepeatSaturatingInput {
                mut update,
            } => Some(update.as_mut().reclaim()?),
            SequenceFrameUpdateInner::SaturatedInput {
                inputs, ..
            } => Some(inputs),
            _ => None,
        };

        for inputs in inputs.into_iter() {
            input_buffer.extend_front(time, inputs)
        }

        Ok(self.outputs_emitted.is_some())
    }

    pub fn poll(&mut self, waker: Waker) -> Result<SequenceFrameUpdatePoll<T>, ()> {
        let mut cx = Context::from_waker(&waker);

        fn handle_original_outputs<T: Transposer>(
            outputs: Vec<T::Output>,
            outputs_emitted: &mut OutputsEmitted,
        ) -> Result<SequenceFrameUpdatePoll<T>, ()> {
            Ok(if outputs.is_empty() {
                *outputs_emitted = OutputsEmitted::None;
                SequenceFrameUpdatePoll::ReadyNoOutputs
            } else {
                *outputs_emitted = OutputsEmitted::Some;
                SequenceFrameUpdatePoll::ReadyOutputs(outputs)
            })
        }

        fn handle_pending<T: Transposer, C: UpdateContext<T>, A: Arg<T>>(
            update: &FrameUpdate<T, C, A>,
        ) -> Result<SequenceFrameUpdatePoll<T>, ()> {
            Ok({
                if update.needs_input_state()? {
                    SequenceFrameUpdatePoll::NeedsState
                } else {
                    SequenceFrameUpdatePoll::Pending
                }
            })
        }

        match &mut self.inner {
            SequenceFrameUpdateInner::SaturatingInit {
                update,
            } => match update.as_mut().poll(&mut cx) {
                Poll::Ready(UpdateResult {
                    frame,
                    outputs,
                    arg: (),
                }) => {
                    self.inner = {
                        SequenceFrameUpdateInner::SaturatedInit {
                            frame,
                        }
                    };

                    handle_original_outputs(outputs, &mut self.outputs_emitted)
                },
                Poll::Pending => handle_pending(update),
            },
            SequenceFrameUpdateInner::OriginalSaturatingInput {
                update,
            } => match update.as_mut().poll(&mut cx) {
                Poll::Ready(UpdateResult {
                    frame,
                    outputs,
                    arg,
                }) => {
                    self.inner = {
                        SequenceFrameUpdateInner::SaturatedInput {
                            frame,
                            inputs: arg,
                        }
                    };

                    handle_original_outputs(outputs, &mut self.outputs_emitted)
                },
                Poll::Pending => handle_pending(update),
            },

            SequenceFrameUpdateInner::RepeatSaturatingInput {
                update,
            } => match update.as_mut().poll(&mut cx) {
                Poll::Ready(UpdateResult {
                    frame,
                    outputs: _,
                    arg,
                }) => {
                    self.inner = {
                        SequenceFrameUpdateInner::SaturatedInput {
                            frame,
                            inputs: arg,
                        }
                    };

                    Ok(SequenceFrameUpdatePoll::ReadyNoOutputs)
                },
                Poll::Pending => handle_pending(update),
            },
            SequenceFrameUpdateInner::OriginalSaturatingScheduled {
                update,
            } => match update.as_mut().poll(&mut cx) {
                Poll::Ready(UpdateResult {
                    frame,
                    outputs,
                    arg: (),
                }) => {
                    self.inner = {
                        SequenceFrameUpdateInner::SaturatedScheduled {
                            frame,
                        }
                    };

                    handle_original_outputs(outputs, &mut self.outputs_emitted)
                },
                Poll::Pending => handle_pending(update),
            },
            SequenceFrameUpdateInner::RepeatSaturatingScheduled {
                update,
            } => match update.as_mut().poll(&mut cx) {
                Poll::Ready(UpdateResult {
                    frame,
                    outputs: _,
                    arg: (),
                }) => {
                    self.inner = {
                        SequenceFrameUpdateInner::SaturatedScheduled {
                            frame,
                        }
                    };

                    Ok(SequenceFrameUpdatePoll::ReadyNoOutputs)
                },
                Poll::Pending => handle_pending(update),
            },
            _ => Err(()),
        }
    }

    pub fn set_input_state(&mut self, state: T::InputState) -> Result<(), T::InputState> {
        match &mut self.inner {
            SequenceFrameUpdateInner::SaturatingInit {
                update,
            } => update.as_mut().set_input_state(state),
            SequenceFrameUpdateInner::OriginalSaturatingInput {
                update,
            } => update.as_mut().set_input_state(state),
            SequenceFrameUpdateInner::OriginalSaturatingScheduled {
                update,
            } => update.as_mut().set_input_state(state),
            SequenceFrameUpdateInner::RepeatSaturatingInput {
                update,
            } => update.as_mut().set_input_state(state),
            SequenceFrameUpdateInner::RepeatSaturatingScheduled {
                update,
            } => update.as_mut().set_input_state(state),
            _ => Err(state),
        }
    }

    pub fn time(&self) -> &EngineTime<T::Time> {
        &self.time
    }
}

enum OutputsEmitted {
    Pending,
    None,
    Some,
}

impl OutputsEmitted {
    pub fn is_some(&self) -> bool {
        matches!(self, &OutputsEmitted::Some)
    }
}

type InitUpdate<T> = FrameUpdate<T, OriginalUpdateContext<T>, InitArg<T>>;
type OriginalInputUpdate<T> = FrameUpdate<T, OriginalUpdateContext<T>, InputArg<T>>;
type OriginalScheduledUpdate<T> = FrameUpdate<T, OriginalUpdateContext<T>, ScheduledArg<T>>;
type RepeatInputUpdate<T> = FrameUpdate<T, RepeatUpdateContext<T>, InputArg<T>>;
type RepeatScheduledUpdate<T> = FrameUpdate<T, RepeatUpdateContext<T>, ScheduledArg<T>>;

enum SequenceFrameUpdateInner<T: Transposer> {
    // notably this can never be rehydrated because you need the preceding frame
    // and there isn't one, because this is init.
    UnsaturatedInit,
    OriginalUnsaturatedInput {
        inputs: <InputArg<T> as Arg<T>>::Stored,
    },
    OriginalUnsaturatedScheduled,
    RepeatUnsaturatedInput {
        inputs: <InputArg<T> as Arg<T>>::Stored,
    },
    RepeatUnsaturatedScheduled,
    SaturatingInit {
        update: Pin<Box<InitUpdate<T>>>,
    },
    OriginalSaturatingInput {
        update: Pin<Box<OriginalInputUpdate<T>>>,
    },
    OriginalSaturatingScheduled {
        update: Pin<Box<OriginalScheduledUpdate<T>>>,
    },
    RepeatSaturatingInput {
        update: Pin<Box<RepeatInputUpdate<T>>>,
    },
    RepeatSaturatingScheduled {
        update: Pin<Box<RepeatScheduledUpdate<T>>>,
    },
    SaturatedInit {
        frame: Box<Frame<T>>,
    },
    SaturatedInput {
        inputs: <InputArg<T> as Arg<T>>::Stored,
        frame:  Box<Frame<T>>,
    },
    SaturatedScheduled {
        frame: Box<Frame<T>>,
    },
    Unreachable,
}

impl<T: Transposer> SequenceFrameUpdateInner<T> {
    pub fn is_unsaturated(&self) -> bool {
        matches!(
            self,
            SequenceFrameUpdateInner::OriginalUnsaturatedInput { .. }
                | SequenceFrameUpdateInner::OriginalUnsaturatedScheduled
                | SequenceFrameUpdateInner::RepeatUnsaturatedInput { .. }
                | SequenceFrameUpdateInner::RepeatUnsaturatedScheduled
        )
    }

    pub fn is_saturated(&self) -> bool {
        matches!(
            self,
            SequenceFrameUpdateInner::SaturatedInit { .. }
                | SequenceFrameUpdateInner::SaturatedInput { .. }
                | SequenceFrameUpdateInner::SaturatedScheduled { .. }
        )
    }
}

#[derive(Debug)]
pub enum SequenceFrameUpdatePoll<T: Transposer> {
    Pending,
    NeedsState,
    ReadyNoOutputs,
    ReadyOutputs(Vec<T::Output>),
}
