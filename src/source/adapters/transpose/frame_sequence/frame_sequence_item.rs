use core::cmp::Ordering;
use core::mem::MaybeUninit;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

use futures_core::Future;

use super::super::engine_time::EngineTime;
use super::super::frame::Frame;
use super::super::frame_update::{FrameUpdate, UpdateResult};
use crate::source::adapters::transpose::frame_update::arg::{Arg, InitArg, InputArg, ScheduledArg};
use crate::source::adapters::transpose::input_buffer::InputBuffer;
use crate::transposer::Transposer;

pub struct FrameSequenceItem<T: Transposer> {
    time:  EngineTime<T::Time>,
    inner: FrameSequenceItemInner<T>,
}

enum FrameSequenceItemInner<T: Transposer> {
    UnsaturatedInput {
        inputs: <InputArg<T> as Arg<T>>::Stored,
    },
    UnsaturatedScheduled,
    SaturatingInit {
        update: Pin<Box<FrameUpdate<T, InitArg<T>>>>,
    },
    SaturatingInput {
        update: Pin<Box<FrameUpdate<T, InputArg<T>>>>,
    },
    SaturatingScheduled {
        update: Pin<Box<FrameUpdate<T, ScheduledArg<T>>>>,
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
}

impl<T: Transposer> FrameSequenceItemInner<T> {
    pub fn is_unsaturated(&self) -> bool {
        match self {
            FrameSequenceItemInner::UnsaturatedInput {
                ..
            } => true,
            FrameSequenceItemInner::UnsaturatedScheduled => true,
            _ => false,
        }
    }
    pub fn is_saturating(&self) -> bool {
        match self {
            FrameSequenceItemInner::SaturatingInit {
                ..
            } => true,
            FrameSequenceItemInner::SaturatingInput {
                ..
            } => true,
            FrameSequenceItemInner::SaturatingScheduled {
                ..
            } => true,
            _ => false,
        }
    }
    pub fn is_saturated(&self) -> bool {
        match self {
            FrameSequenceItemInner::SaturatedInit {
                ..
            } => true,
            FrameSequenceItemInner::SaturatedInput {
                ..
            } => true,
            FrameSequenceItemInner::SaturatedScheduled {
                ..
            } => true,
            _ => false,
        }
    }

    pub fn can_be_desaturated(&self) -> bool {
        match self {
            FrameSequenceItemInner::SaturatedInput {
                ..
            } => true,
            FrameSequenceItemInner::SaturatedScheduled {
                ..
            } => true,
            _ => false,
        }
    }
}

impl<T: Transposer> FrameSequenceItem<T> {
    #[allow(unused)]
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
        }
    }

    #[allow(unused)]
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
                inner: FrameSequenceItemInner::UnsaturatedInput {
                    inputs: input_buffer.pop_first().unwrap().1,
                },
            }
        } else {
            FrameSequenceItem {
                time,
                inner: FrameSequenceItemInner::UnsaturatedScheduled,
            }
        };

        Ok(Some(item))
    }

    #[allow(unused)]
    pub fn saturate_take(&mut self, previous: &mut Self) -> Result<(), ()> {
        if !(previous.inner.can_be_desaturated() && self.inner.is_unsaturated()) {
            return Err(())
        }

        let mut frame_dest = MaybeUninit::uninit();

        take_mut::take(&mut previous.inner, |prev| match prev {
            FrameSequenceItemInner::SaturatedInput {
                inputs,
                frame,
            } => {
                frame_dest = MaybeUninit::new(frame);
                FrameSequenceItemInner::UnsaturatedInput {
                    inputs,
                }
            },
            FrameSequenceItemInner::SaturatedScheduled {
                frame,
            } => {
                frame_dest = MaybeUninit::new(frame);
                FrameSequenceItemInner::UnsaturatedScheduled
            },
            _ => unreachable!(),
        });

        let frame = unsafe { frame_dest.assume_init() };

        self.saturate_from_frame(frame)?;

        Ok(())
    }

    #[allow(unused)]
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

    #[allow(unused)]
    pub fn desaturate(&mut self) -> Result<(), ()> {
        let mut result = Err(());
        take_mut::take(&mut self.inner, |original| match original {
            FrameSequenceItemInner::SaturatingInput {
                mut update,
            } => {
                let inputs = update.as_mut().reclaim();
                if inputs.is_ok() {
                    result = Ok(())
                };
                FrameSequenceItemInner::UnsaturatedInput {
                    inputs: inputs.unwrap_or(Box::new([])),
                }
            },
            FrameSequenceItemInner::SaturatingScheduled {
                update,
            } => {
                result = Ok(());
                FrameSequenceItemInner::UnsaturatedScheduled
            },
            FrameSequenceItemInner::SaturatedInput {
                inputs, ..
            } => {
                result = Ok(());
                FrameSequenceItemInner::UnsaturatedInput {
                    inputs,
                }
            },
            FrameSequenceItemInner::SaturatedScheduled {
                ..
            } => {
                result = Ok(());
                FrameSequenceItemInner::UnsaturatedScheduled
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
            FrameSequenceItemInner::UnsaturatedInput {
                inputs,
            } => Some(inputs),
            FrameSequenceItemInner::SaturatingInput {
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

        todo!() // need to keep track of what has beem emitted
    }

    #[allow(unused)]
    pub fn poll(&mut self, waker: Waker) -> Result<FrameSequenceItemInnerPoll<T>, ()> {
        let mut cx = Context::from_waker(&waker);
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

                    Ok(FrameSequenceItemInnerPoll::Ready(outputs))
                },
                Poll::Pending => Ok({
                    if update.needs_input_state()? {
                        FrameSequenceItemInnerPoll::NeedsState
                    } else {
                        FrameSequenceItemInnerPoll::Pending
                    }
                }),
            },
            FrameSequenceItemInner::SaturatingInput {
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

                    Ok(FrameSequenceItemInnerPoll::Ready(outputs))
                },
                Poll::Pending => Ok({
                    if update.needs_input_state()? {
                        FrameSequenceItemInnerPoll::NeedsState
                    } else {
                        FrameSequenceItemInnerPoll::Pending
                    }
                }),
            },
            FrameSequenceItemInner::SaturatingScheduled {
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

                    Ok(FrameSequenceItemInnerPoll::Ready(outputs))
                },
                Poll::Pending => Ok({
                    if update.needs_input_state()? {
                        FrameSequenceItemInnerPoll::NeedsState
                    } else {
                        FrameSequenceItemInnerPoll::Pending
                    }
                }),
            },
            _ => Err(()),
        }
    }

    #[allow(unused)]
    pub fn time(&self) -> &EngineTime<T::Time> {
        &self.time
    }

    fn saturate_from_frame(&mut self, frame: Box<Frame<T>>) -> Result<(), ()> {
        if !self.inner.is_unsaturated() {
            return Err(())
        }

        take_mut::take(&mut self.inner, |next| match next {
            FrameSequenceItemInner::UnsaturatedInput {
                inputs,
            } => {
                let update = FrameUpdate::new(frame, inputs, self.time.clone());
                FrameSequenceItemInner::SaturatingInput {
                    update: Box::pin(update),
                }
            },
            FrameSequenceItemInner::UnsaturatedScheduled => {
                let update = FrameUpdate::new(frame, (), self.time.clone());
                FrameSequenceItemInner::SaturatingScheduled {
                    update: Box::pin(update),
                }
            },
            _ => unreachable!(),
        });

        Ok(())
    }
}

pub enum FrameSequenceItemInnerPoll<T: Transposer> {
    Pending,
    NeedsState,
    Ready(Vec<T::Output>),
}
