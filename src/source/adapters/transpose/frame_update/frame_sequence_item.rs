use core::cmp::Ordering;
use core::mem::MaybeUninit;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use std::process::Output;

use futures_core::Future;

use super::super::engine_time::EngineTime;
use super::super::frame::Frame;
use super::super::frame_update::{FrameUpdate, UpdateResult};
use super::update_context::{OriginalUpdateContext, RepeatUpdateContext};
use crate::source::adapters::transpose::frame_update::arg::{Arg, InitArg, InputArg, ScheduledArg};
use crate::source::adapters::transpose::frame_update::update_context::UpdateContext;
use crate::source::adapters::transpose::input_buffer::InputBuffer;
use crate::transposer::Transposer;

pub struct FrameSequenceItem<T: Transposer> {
    time:            EngineTime<T::Time>,
    inner:           FrameSequenceItemInner<T>,
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

enum FrameSequenceItemInner<T: Transposer> {
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

impl<T: Transposer> FrameSequenceItemInner<T> {
    pub fn is_unsaturated(&self) -> bool {
        matches!(
            self,
            FrameSequenceItemInner::OriginalUnsaturatedInput { .. }
                | FrameSequenceItemInner::OriginalUnsaturatedScheduled
                | FrameSequenceItemInner::RepeatUnsaturatedInput { .. }
                | FrameSequenceItemInner::RepeatUnsaturatedScheduled
        )
    }
    pub fn is_saturating(&self) -> bool {
        matches!(
            self,
            FrameSequenceItemInner::SaturatingInit { .. }
                | FrameSequenceItemInner::OriginalSaturatingInput { .. }
                | FrameSequenceItemInner::OriginalSaturatingScheduled { .. }
                | FrameSequenceItemInner::RepeatSaturatingInput { .. }
                | FrameSequenceItemInner::RepeatSaturatingScheduled { .. }
        )
    }
    pub fn is_saturated(&self) -> bool {
        matches!(
            self,
            FrameSequenceItemInner::SaturatedInit { .. }
                | FrameSequenceItemInner::SaturatedInput { .. }
                | FrameSequenceItemInner::SaturatedScheduled { .. }
        )
    }

    pub fn can_be_taken(&self) -> bool {
        matches!(
            self,
            FrameSequenceItemInner::SaturatedInput { .. }
                | FrameSequenceItemInner::SaturatedScheduled { .. }
        )
    }
}

impl<T: Transposer> FrameSequenceItem<T> {
    pub fn new_init(transposer: T, rng_seed: [u8; 32]) -> Self {
        let time = EngineTime::new_init();
        let frame = Frame::new(transposer, rng_seed);
        let frame = Box::new(frame);
        let update = FrameUpdate::new(frame, (), time.clone());
        let update = Box::pin(update);
        let inner = FrameSequenceItemInner::SaturatingInit {
            update,
        };
        FrameSequenceItem {
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
            FrameSequenceItemInner::SaturatedInput {
                frame, ..
            } => frame.as_ref(),
            FrameSequenceItemInner::SaturatedScheduled {
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
            FrameSequenceItem {
                time,
                inner: FrameSequenceItemInner::OriginalUnsaturatedInput {
                    inputs: input_buffer.pop_first().unwrap().1,
                },
                outputs_emitted: OutputsEmitted::Pending,
            }
        } else {
            FrameSequenceItem {
                time,
                inner: FrameSequenceItemInner::OriginalUnsaturatedScheduled,
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
            || FrameSequenceItemInner::Unreachable,
            |prev| match prev {
                FrameSequenceItemInner::SaturatedInput {
                    inputs,
                    frame,
                } => {
                    frame_dest = MaybeUninit::new(frame);
                    FrameSequenceItemInner::RepeatUnsaturatedInput {
                        inputs,
                    }
                },
                FrameSequenceItemInner::SaturatedScheduled {
                    frame,
                } => {
                    frame_dest = MaybeUninit::new(frame);
                    FrameSequenceItemInner::RepeatUnsaturatedScheduled
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
            FrameSequenceItemInner::SaturatedInit {
                frame, ..
            } => Ok(frame.clone()),
            FrameSequenceItemInner::SaturatedInput {
                frame, ..
            } => Ok(frame.clone()),
            FrameSequenceItemInner::SaturatedScheduled {
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
            FrameSequenceItemInner::OriginalSaturatingInput {
                mut update,
            } => {
                let inputs = update.as_mut().reclaim();
                if inputs.is_ok() {
                    result = Ok(())
                };
                FrameSequenceItemInner::OriginalUnsaturatedInput {
                    inputs: inputs.unwrap_or(Box::new([])),
                }
            },
            FrameSequenceItemInner::RepeatSaturatingInput {
                mut update,
            } => {
                let inputs = update.as_mut().reclaim();
                if inputs.is_ok() {
                    result = Ok(())
                };
                FrameSequenceItemInner::RepeatUnsaturatedInput {
                    inputs: inputs.unwrap_or(Box::new([])),
                }
            },
            FrameSequenceItemInner::OriginalSaturatingScheduled {
                update,
            } => {
                result = Ok(());
                FrameSequenceItemInner::OriginalUnsaturatedScheduled
            },
            FrameSequenceItemInner::RepeatSaturatingScheduled {
                update,
            } => {
                result = Ok(());
                FrameSequenceItemInner::RepeatUnsaturatedScheduled
            },
            FrameSequenceItemInner::SaturatedInput {
                inputs, ..
            } => {
                result = Ok(());
                FrameSequenceItemInner::RepeatUnsaturatedInput {
                    inputs,
                }
            },
            FrameSequenceItemInner::SaturatedScheduled {
                ..
            } => {
                result = Ok(());
                FrameSequenceItemInner::RepeatUnsaturatedScheduled
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
            FrameSequenceItemInner::OriginalUnsaturatedInput {
                inputs,
            } => Some(inputs),
            FrameSequenceItemInner::RepeatUnsaturatedInput {
                inputs,
            } => Some(inputs),
            FrameSequenceItemInner::OriginalSaturatingInput {
                mut update,
            } => Some(update.as_mut().reclaim()?),
            FrameSequenceItemInner::RepeatSaturatingInput {
                mut update,
            } => Some(update.as_mut().reclaim()?),
            FrameSequenceItemInner::SaturatedInput {
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

    pub fn poll(&mut self, waker: Waker) -> Result<FrameSequenceItemPoll<T>, ()> {
        let mut cx = Context::from_waker(&waker);

        fn handle_original_outputs<T: Transposer>(
            outputs: Vec<T::Output>,
            outputs_emitted: &mut OutputsEmitted,
        ) -> Result<FrameSequenceItemPoll<T>, ()> {
            Ok(if outputs.is_empty() {
                *outputs_emitted = OutputsEmitted::None;
                FrameSequenceItemPoll::ReadyNoOutputs
            } else {
                *outputs_emitted = OutputsEmitted::Some;
                FrameSequenceItemPoll::ReadyOutputs(outputs)
            })
        }
        fn handle_repeat_outputs<T: Transposer>(
            outputs: Vec<T::Output>,
            outputs_emitted: &mut OutputsEmitted,
        ) -> Result<FrameSequenceItemPoll<T>, ()> {
            Ok(FrameSequenceItemPoll::ReadyNoOutputs)
        }

        fn handle_pending<T: Transposer, C: UpdateContext<T>, A: Arg<T>>(
            update: &FrameUpdate<T, C, A>,
        ) -> Result<FrameSequenceItemPoll<T>, ()> {
            Ok({
                if update.needs_input_state()? {
                    FrameSequenceItemPoll::NeedsState
                } else {
                    FrameSequenceItemPoll::Pending
                }
            })
        }

        match &mut self.inner {
            FrameSequenceItemInner::SaturatingInit {
                update,
            } => match update.as_mut().poll(&mut cx) {
                Poll::Ready(UpdateResult {
                    frame,
                    outputs,
                    arg: (),
                }) => {
                    self.inner = {
                        FrameSequenceItemInner::SaturatedInit {
                            frame,
                        }
                    };

                    handle_original_outputs(outputs, &mut self.outputs_emitted)
                },
                Poll::Pending => handle_pending(update),
            },
            FrameSequenceItemInner::OriginalSaturatingInput {
                update,
            } => match update.as_mut().poll(&mut cx) {
                Poll::Ready(UpdateResult {
                    frame,
                    outputs,
                    arg,
                }) => {
                    self.inner = {
                        FrameSequenceItemInner::SaturatedInput {
                            frame,
                            inputs: arg,
                        }
                    };

                    handle_original_outputs(outputs, &mut self.outputs_emitted)
                },
                Poll::Pending => handle_pending(update),
            },

            FrameSequenceItemInner::RepeatSaturatingInput {
                update,
            } => match update.as_mut().poll(&mut cx) {
                Poll::Ready(UpdateResult {
                    frame,
                    outputs,
                    arg,
                }) => {
                    self.inner = {
                        FrameSequenceItemInner::SaturatedInput {
                            frame,
                            inputs: arg,
                        }
                    };

                    Ok(FrameSequenceItemPoll::ReadyNoOutputs)
                },
                Poll::Pending => handle_pending(update),
            },
            FrameSequenceItemInner::OriginalSaturatingScheduled {
                update,
            } => match update.as_mut().poll(&mut cx) {
                Poll::Ready(UpdateResult {
                    frame,
                    outputs,
                    arg: (),
                }) => {
                    self.inner = {
                        FrameSequenceItemInner::SaturatedScheduled {
                            frame,
                        }
                    };

                    handle_original_outputs(outputs, &mut self.outputs_emitted)
                },
                Poll::Pending => handle_pending(update),
            },
            FrameSequenceItemInner::RepeatSaturatingScheduled {
                update,
            } => match update.as_mut().poll(&mut cx) {
                Poll::Ready(UpdateResult {
                    frame,
                    outputs,
                    arg: (),
                }) => {
                    self.inner = {
                        FrameSequenceItemInner::SaturatedScheduled {
                            frame,
                        }
                    };

                    Ok(FrameSequenceItemPoll::ReadyNoOutputs)
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
            || FrameSequenceItemInner::Unreachable,
            |next| match next {
                FrameSequenceItemInner::OriginalUnsaturatedInput {
                    inputs,
                } => {
                    let update = FrameUpdate::new(frame, inputs, self.time.clone());
                    FrameSequenceItemInner::OriginalSaturatingInput {
                        update: Box::pin(update),
                    }
                },
                FrameSequenceItemInner::RepeatUnsaturatedInput {
                    inputs,
                } => {
                    let update = FrameUpdate::new(frame, inputs, self.time.clone());
                    FrameSequenceItemInner::RepeatSaturatingInput {
                        update: Box::pin(update),
                    }
                },
                FrameSequenceItemInner::OriginalUnsaturatedScheduled => {
                    let update = FrameUpdate::new(frame, (), self.time.clone());
                    FrameSequenceItemInner::OriginalSaturatingScheduled {
                        update: Box::pin(update),
                    }
                },
                FrameSequenceItemInner::RepeatUnsaturatedScheduled => {
                    let update = FrameUpdate::new(frame, (), self.time.clone());
                    FrameSequenceItemInner::RepeatSaturatingScheduled {
                        update: Box::pin(update),
                    }
                },
                _ => unreachable!(),
            },
        );

        Ok(())
    }
}

pub enum FrameSequenceItemPoll<T: Transposer> {
    Pending,
    NeedsState,
    ReadyNoOutputs,
    ReadyOutputs(Vec<T::Output>),
}
