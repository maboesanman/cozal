use core::cmp::Ordering;
use core::mem::{replace, MaybeUninit};
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

use futures_core::Future;

use super::args::{InitArg, InputArg, ScheduledArg};
use super::engine_time::EngineTime;
use super::frame_update::{Frame, FrameUpdate};
use super::update_context_collector::UpdateContextCollector;
use crate::source::adapters::transpose::sequence_frame::frame_update::{
    Arg,
    UpdateContext,
    UpdateResult,
};
use crate::transposer::Transposer;

pub struct SequenceFrame<T: Transposer> {
    time:            EngineTime<T::Time>,
    inner:           SequenceFrameInner<T>,
    outputs_emitted: OutputsEmitted,

    // these are used purely for enforcing that saturate calls use the previous frame.
    #[cfg(debug_assertions)]
    uuid_self: uuid::Uuid,
    #[cfg(debug_assertions)]
    uuid_prev: Option<uuid::Uuid>,
}

impl<T: Transposer> SequenceFrame<T> {
    pub fn new_init(transposer: T, rng_seed: [u8; 32]) -> Self {
        let time = EngineTime::new_init();
        let frame = Frame::new(transposer, rng_seed);
        let frame = Box::new(frame);
        let update = FrameUpdate::new(frame, (), time.clone());
        let update = Box::pin(update);
        let inner = SequenceFrameInner::SaturatingInit {
            update,
        };
        SequenceFrame {
            time,
            inner,
            outputs_emitted: OutputsEmitted::Pending,

            #[cfg(debug_assertions)]
            uuid_self: uuid::Uuid::new_v4(),
            #[cfg(debug_assertions)]
            uuid_prev: None,
        }
    }

    pub fn next_unsaturated(
        &self,
        next_inputs: &mut Option<(T::Time, Box<[T::Input]>)>,
    ) -> Result<Option<Self>, ()> {
        let frame = match &self.inner {
            SequenceFrameInner::SaturatedInit {
                frame, ..
            } => frame.as_ref(),
            SequenceFrameInner::SaturatedInput {
                frame, ..
            } => frame.as_ref(),
            SequenceFrameInner::SaturatedScheduled {
                frame, ..
            } => frame.as_ref(),
            _ => return Err(()),
        };
        let next_scheduled_time = frame.get_next_scheduled_time();

        let next_time_index = self.time.index() + 1;

        let (time, is_input) = match (&next_inputs, next_scheduled_time) {
            (None, None) => return Ok(None),
            (None, Some(t)) => (EngineTime::new_scheduled(next_time_index, t.clone()), false),
            (Some((t, _)), None) => (EngineTime::new_input(next_time_index, *t), true),
            (Some((t_i, _)), Some(t_s)) => match t_i.cmp(&t_s.time) {
                Ordering::Greater => (
                    EngineTime::new_scheduled(next_time_index, t_s.clone()),
                    false,
                ),
                _ => (EngineTime::new_input(next_time_index, *t_i), true),
            },
        };

        let inner = if is_input {
            SequenceFrameInner::OriginalUnsaturatedInput {
                inputs: core::mem::take(next_inputs).unwrap().1,
            }
        } else {
            SequenceFrameInner::OriginalUnsaturatedScheduled
        };

        let item = SequenceFrame {
            time,
            inner,
            outputs_emitted: OutputsEmitted::Pending,

            #[cfg(debug_assertions)]
            uuid_self: uuid::Uuid::new_v4(),
            #[cfg(debug_assertions)]
            uuid_prev: Some(self.uuid_self),
        };

        Ok(Some(item))
    }

    // previous is expected to be the value produced this via next_unsaturated.
    pub fn saturate_take(&mut self, previous: &mut Self) -> Result<(), ()> {
        #[cfg(debug_assertions)]
        debug_assert!(self.uuid_prev == Some(previous.uuid_self));

        if !(previous.inner.is_saturated() && self.inner.is_unsaturated()) {
            return Err(())
        }

        let mut frame_dest = MaybeUninit::uninit();

        take_mut::take_or_recover(
            &mut previous.inner,
            || SequenceFrameInner::Unreachable,
            |prev| match prev {
                SequenceFrameInner::SaturatedInit {
                    frame,
                } => {
                    frame_dest = MaybeUninit::new(frame);
                    SequenceFrameInner::UnsaturatedInit
                },
                SequenceFrameInner::SaturatedInput {
                    inputs,
                    frame,
                } => {
                    frame_dest = MaybeUninit::new(frame);
                    SequenceFrameInner::RepeatUnsaturatedInput {
                        inputs,
                    }
                },
                SequenceFrameInner::SaturatedScheduled {
                    frame,
                } => {
                    frame_dest = MaybeUninit::new(frame);
                    SequenceFrameInner::RepeatUnsaturatedScheduled
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
        #[cfg(debug_assertions)]
        debug_assert!(self.uuid_prev == Some(previous.uuid_self));

        let frame = match &previous.inner {
            SequenceFrameInner::SaturatedInit {
                frame, ..
            } => Ok(frame.clone()),
            SequenceFrameInner::SaturatedInput {
                frame, ..
            } => Ok(frame.clone()),
            SequenceFrameInner::SaturatedScheduled {
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
            || SequenceFrameInner::Unreachable,
            |next| match next {
                SequenceFrameInner::OriginalUnsaturatedInput {
                    inputs,
                } => {
                    let update = FrameUpdate::new(frame, inputs, self.time.clone());
                    SequenceFrameInner::OriginalSaturatingInput {
                        update: Box::pin(update),
                    }
                },
                SequenceFrameInner::RepeatUnsaturatedInput {
                    inputs,
                } => {
                    let update = FrameUpdate::new(frame, inputs, self.time.clone());
                    SequenceFrameInner::RepeatSaturatingInput {
                        update: Box::pin(update),
                    }
                },
                SequenceFrameInner::OriginalUnsaturatedScheduled => {
                    let update = FrameUpdate::new(frame, (), self.time.clone());
                    SequenceFrameInner::OriginalSaturatingScheduled {
                        update: Box::pin(update),
                    }
                },
                SequenceFrameInner::RepeatUnsaturatedScheduled => {
                    let update = FrameUpdate::new(frame, (), self.time.clone());
                    SequenceFrameInner::RepeatSaturatingScheduled {
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
            SequenceFrameInner::OriginalSaturatingInput {
                mut update,
            } => {
                let inputs = update.as_mut().reclaim();
                if inputs.is_ok() {
                    result = Ok(())
                };
                SequenceFrameInner::OriginalUnsaturatedInput {
                    inputs: inputs.unwrap_or(Box::new([])),
                }
            },
            SequenceFrameInner::RepeatSaturatingInput {
                mut update,
            } => {
                let inputs = update.as_mut().reclaim();
                if inputs.is_ok() {
                    result = Ok(())
                };
                SequenceFrameInner::RepeatUnsaturatedInput {
                    inputs: inputs.unwrap_or(Box::new([])),
                }
            },
            SequenceFrameInner::OriginalSaturatingScheduled {
                update: _,
            } => {
                result = Ok(());
                SequenceFrameInner::OriginalUnsaturatedScheduled
            },
            SequenceFrameInner::RepeatSaturatingScheduled {
                update: _,
            } => {
                result = Ok(());
                SequenceFrameInner::RepeatUnsaturatedScheduled
            },
            SequenceFrameInner::SaturatedInit {
                ..
            } => {
                result = Ok(());
                SequenceFrameInner::UnsaturatedInit
            },
            SequenceFrameInner::SaturatedInput {
                inputs, ..
            } => {
                result = Ok(());
                SequenceFrameInner::RepeatUnsaturatedInput {
                    inputs,
                }
            },
            SequenceFrameInner::SaturatedScheduled {
                ..
            } => {
                result = Ok(());
                SequenceFrameInner::RepeatUnsaturatedScheduled
            },
            other => other,
        });

        result
    }

    pub fn rollback(mut self) -> Result<(bool, Option<Box<[T::Input]>>), ()> {
        let inputs = match replace(&mut self.inner, SequenceFrameInner::Unreachable) {
            SequenceFrameInner::OriginalUnsaturatedInput {
                inputs,
            } => Some(inputs),
            SequenceFrameInner::RepeatUnsaturatedInput {
                inputs,
            } => Some(inputs),
            SequenceFrameInner::OriginalSaturatingInput {
                mut update,
            } => Some(update.as_mut().reclaim()?),
            SequenceFrameInner::RepeatSaturatingInput {
                mut update,
            } => Some(update.as_mut().reclaim()?),
            SequenceFrameInner::SaturatedInput {
                inputs, ..
            } => Some(inputs),
            _ => None,
        };

        Ok((self.outputs_emitted.is_some(), inputs))
    }

    pub fn poll(&mut self, waker: Waker) -> Result<SequenceFramePoll<T>, ()> {
        let mut cx = Context::from_waker(&waker);

        fn handle_original_outputs<T: Transposer>(
            outputs: Vec<T::Output>,
            outputs_emitted: &mut OutputsEmitted,
        ) -> Result<SequenceFramePoll<T>, ()> {
            Ok(if outputs.is_empty() {
                *outputs_emitted = OutputsEmitted::None;
                SequenceFramePoll::ReadyNoOutputs
            } else {
                *outputs_emitted = OutputsEmitted::Some;
                SequenceFramePoll::ReadyOutputs(outputs)
            })
        }

        fn handle_pending<T: Transposer, C: UpdateContext<T>, A: Arg<T>>(
            update: &FrameUpdate<T, C, A>,
        ) -> Result<SequenceFramePoll<T>, ()> {
            Ok({
                if update.needs_input_state()? {
                    SequenceFramePoll::NeedsState
                } else {
                    SequenceFramePoll::Pending
                }
            })
        }

        match &mut self.inner {
            SequenceFrameInner::SaturatingInit {
                update,
            } => match update.as_mut().poll(&mut cx) {
                Poll::Ready(UpdateResult {
                    frame,
                    outputs,
                    arg: (),
                }) => {
                    self.inner = {
                        SequenceFrameInner::SaturatedInit {
                            frame,
                        }
                    };

                    handle_original_outputs(outputs, &mut self.outputs_emitted)
                },
                Poll::Pending => handle_pending(update),
            },
            SequenceFrameInner::OriginalSaturatingInput {
                update,
            } => match update.as_mut().poll(&mut cx) {
                Poll::Ready(UpdateResult {
                    frame,
                    outputs,
                    arg,
                }) => {
                    self.inner = {
                        SequenceFrameInner::SaturatedInput {
                            frame,
                            inputs: arg,
                        }
                    };

                    handle_original_outputs(outputs, &mut self.outputs_emitted)
                },
                Poll::Pending => handle_pending(update),
            },

            SequenceFrameInner::RepeatSaturatingInput {
                update,
            } => match update.as_mut().poll(&mut cx) {
                Poll::Ready(UpdateResult {
                    frame,
                    outputs: _,
                    arg,
                }) => {
                    self.inner = {
                        SequenceFrameInner::SaturatedInput {
                            frame,
                            inputs: arg,
                        }
                    };

                    Ok(SequenceFramePoll::ReadyNoOutputs)
                },
                Poll::Pending => handle_pending(update),
            },
            SequenceFrameInner::OriginalSaturatingScheduled {
                update,
            } => match update.as_mut().poll(&mut cx) {
                Poll::Ready(UpdateResult {
                    frame,
                    outputs,
                    arg: (),
                }) => {
                    self.inner = {
                        SequenceFrameInner::SaturatedScheduled {
                            frame,
                        }
                    };

                    handle_original_outputs(outputs, &mut self.outputs_emitted)
                },
                Poll::Pending => handle_pending(update),
            },
            SequenceFrameInner::RepeatSaturatingScheduled {
                update,
            } => match update.as_mut().poll(&mut cx) {
                Poll::Ready(UpdateResult {
                    frame,
                    outputs: _,
                    arg: (),
                }) => {
                    self.inner = {
                        SequenceFrameInner::SaturatedScheduled {
                            frame,
                        }
                    };

                    Ok(SequenceFramePoll::ReadyNoOutputs)
                },
                Poll::Pending => handle_pending(update),
            },
            _ => Err(()),
        }
    }

    pub fn set_input_state(&mut self, state: T::InputState) -> Result<(), T::InputState> {
        match &mut self.inner {
            SequenceFrameInner::SaturatingInit {
                update,
            } => update.as_mut().set_input_state(state),
            SequenceFrameInner::OriginalSaturatingInput {
                update,
            } => update.as_mut().set_input_state(state),
            SequenceFrameInner::OriginalSaturatingScheduled {
                update,
            } => update.as_mut().set_input_state(state),
            SequenceFrameInner::RepeatSaturatingInput {
                update,
            } => update.as_mut().set_input_state(state),
            SequenceFrameInner::RepeatSaturatingScheduled {
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

type OriginalContext<T> = UpdateContextCollector<T, Vec<<T as Transposer>::Output>>;
type RepeatContext<T> = UpdateContextCollector<T, ()>;

type InitUpdate<T> = FrameUpdate<T, OriginalContext<T>, InitArg<T>>;
type InputUpdate<T, C> = FrameUpdate<T, C, InputArg<T>>;
type ScheduledUpdate<T, C> = FrameUpdate<T, C, ScheduledArg<T>>;

type OriginalInputUpdate<T> = InputUpdate<T, OriginalContext<T>>;
type OriginalScheduledUpdate<T> = ScheduledUpdate<T, OriginalContext<T>>;
type RepeatInputUpdate<T> = InputUpdate<T, RepeatContext<T>>;
type RepeatScheduledUpdate<T> = ScheduledUpdate<T, RepeatContext<T>>;

enum SequenceFrameInner<T: Transposer> {
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

impl<T: Transposer> SequenceFrameInner<T> {
    pub fn is_unsaturated(&self) -> bool {
        matches!(
            self,
            SequenceFrameInner::OriginalUnsaturatedInput { .. }
                | SequenceFrameInner::OriginalUnsaturatedScheduled
                | SequenceFrameInner::RepeatUnsaturatedInput { .. }
                | SequenceFrameInner::RepeatUnsaturatedScheduled
        )
    }

    pub fn is_saturated(&self) -> bool {
        matches!(
            self,
            SequenceFrameInner::SaturatedInit { .. }
                | SequenceFrameInner::SaturatedInput { .. }
                | SequenceFrameInner::SaturatedScheduled { .. }
        )
    }
}

#[derive(Debug)]
pub enum SequenceFramePoll<T: Transposer> {
    Pending,
    NeedsState,
    ReadyNoOutputs,
    ReadyOutputs(Vec<T::Output>),
}
