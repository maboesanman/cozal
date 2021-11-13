use core::cmp::Ordering;
use core::mem::MaybeUninit;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use std::process::Output;

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
    pub fn is_saturating(&self) -> bool {
        matches!(
            self,
            SequenceFrameUpdateInner::SaturatingInit { .. }
                | SequenceFrameUpdateInner::OriginalSaturatingInput { .. }
                | SequenceFrameUpdateInner::OriginalSaturatingScheduled { .. }
                | SequenceFrameUpdateInner::RepeatSaturatingInput { .. }
                | SequenceFrameUpdateInner::RepeatSaturatingScheduled { .. }
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

    pub fn can_be_taken(&self) -> bool {
        matches!(
            self,
            SequenceFrameUpdateInner::SaturatedInput { .. }
                | SequenceFrameUpdateInner::SaturatedScheduled { .. }
        )
    }
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

        let (time, is_input) = match (next_input_time, next_scheduled_time) {
            (None, None) => return Ok(None),
            (None, Some(t)) => (EngineTime::new_scheduled(t.clone()), false),
            (Some(t), None) => (EngineTime::new_input(t), true),
            (Some(t_i), Some(t_s)) => match t_i.cmp(&t_s.time) {
                Ordering::Greater => (EngineTime::new_scheduled(t_s.clone()), false),
                _ => (EngineTime::new_input(t_i), true),
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

    pub fn saturate_take(&mut self, previous: &mut Self) -> Result<(), ()> {
        if !(previous.inner.can_be_taken() && self.inner.is_unsaturated()) {
            return Err(())
        }

        let mut frame_dest = MaybeUninit::uninit();

        take_mut::take_or_recover(
            &mut previous.inner,
            || SequenceFrameUpdateInner::Unreachable,
            |prev| match prev {
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
                update,
            } => {
                result = Ok(());
                SequenceFrameUpdateInner::OriginalUnsaturatedScheduled
            },
            SequenceFrameUpdateInner::RepeatSaturatingScheduled {
                update,
            } => {
                result = Ok(());
                SequenceFrameUpdateInner::RepeatUnsaturatedScheduled
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
        self,
        input_buffer: &mut InputBuffer<T::Time, T::Input>,
    ) -> Result<Option<T::Time>, ()> {
        let time = self.time.raw_time().map_err(|_| ())?;
        let inputs = match self.inner {
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

        if self.outputs_emitted.is_some() {
            // do something?
        }
        todo!() // need to keep track of what has beem emitted
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
        fn handle_repeat_outputs<T: Transposer>(
            outputs: Vec<T::Output>,
            outputs_emitted: &mut OutputsEmitted,
        ) -> Result<SequenceFrameUpdatePoll<T>, ()> {
            Ok(SequenceFrameUpdatePoll::ReadyNoOutputs)
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
                    outputs,
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
                    outputs,
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

    pub fn time(&self) -> &EngineTime<T::Time> {
        &self.time
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
}

pub enum SequenceFrameUpdatePoll<T: Transposer> {
    Pending,
    NeedsState,
    ReadyNoOutputs,
    ReadyOutputs(Vec<T::Output>),
}
