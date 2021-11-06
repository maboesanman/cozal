use core::pin::Pin;
use std::cmp::Ordering;
use std::mem::MaybeUninit;
use std::task::{Context, Poll, Waker};

use futures_core::Future;

use super::super::engine_time::EngineTime;
use super::super::frame::Frame;
use super::super::frame_update::{FrameUpdate, UpdateResult};
use crate::source::adapters::transpose::input_buffer::InputBuffer;
use crate::transposer::Transposer;

pub struct FrameSequenceItem<T: Transposer> {
    time:  EngineTime<T::Time>,
    inner: FrameSequenceItemInner<T>,
}

enum FrameSequenceItemInner<T: Transposer> {
    UnsaturatedInput {
        inputs: Vec<T::Input>,
    },
    UnsaturatedScheduled,
    Saturating {
        update: Pin<Box<FrameUpdate<T>>>,
    },
    SaturatedInput {
        inputs: Vec<T::Input>,
        frame:  Box<Frame<T>>,
    },
    SaturatedScheduled {
        frame: Box<Frame<T>>,
    },
}

impl<T: Transposer> FrameSequenceItem<T> {
    #[allow(unused)]
    pub fn new_init(transposer: T, rng_seed: [u8; 32]) -> Self {
        let time = EngineTime::new_init();
        let frame = Frame::new(transposer, rng_seed);
        let frame = Box::new(frame);
        let update = FrameUpdate::new_init(frame, time.clone());
        let update = Box::pin(update);
        let inner = FrameSequenceItemInner::Saturating {
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
        self.saturate_check(previous)?;

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

        unsafe {
            self.saturate_from_frame(frame);
        }

        Ok(())
    }

    #[allow(unused)]
    pub fn saturate_clone(&mut self, previous: &Self) -> Result<(), ()>
    where
        T: Clone,
    {
        self.saturate_check(previous)?;

        let frame = match &previous.inner {
            FrameSequenceItemInner::SaturatedInput {
                frame, ..
            } => frame.clone(),
            FrameSequenceItemInner::SaturatedScheduled {
                frame,
            } => frame.clone(),
            _ => unreachable!(),
        };

        unsafe {
            self.saturate_from_frame(frame);
        }

        Ok(())
    }

    #[allow(unused)]
    pub fn desaturate(&mut self) -> Result<(), ()> {
        let mut result = Err(());
        take_mut::take(&mut self.inner, |original| match original {
            FrameSequenceItemInner::Saturating {
                mut update,
            } => {
                result = Ok(());
                match update.as_mut().reclaim() {
                    Some(inputs) => FrameSequenceItemInner::UnsaturatedInput {
                        inputs,
                    },
                    None => FrameSequenceItemInner::UnsaturatedScheduled,
                }
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

    #[allow(unused)]
    pub fn poll(&mut self, waker: Waker) -> Result<FrameSequenceItemInnerPoll<T>, ()> {
        if let FrameSequenceItemInner::Saturating {
            update,
        } = &mut self.inner
        {
            let mut context = Context::from_waker(&waker);

            match update.as_mut().poll(&mut context) {
                Poll::Ready(UpdateResult {
                    frame,
                    outputs,
                    inputs,
                    exit,
                }) => {
                    self.inner = match inputs {
                        Some(inputs) => FrameSequenceItemInner::SaturatedInput {
                            inputs,
                            frame,
                        },
                        None => FrameSequenceItemInner::SaturatedScheduled {
                            frame,
                        },
                    };

                    drop(exit);

                    Ok(FrameSequenceItemInnerPoll::Ready(outputs))
                },
                Poll::Pending => Ok({
                    if update.needs_input_state()? {
                        FrameSequenceItemInnerPoll::NeedsState
                    } else {
                        FrameSequenceItemInnerPoll::Pending
                    }
                }),
            }
        } else {
            Err(())
        }
    }

    pub fn time(&self) -> &EngineTime<T::Time> {
        &self.time
    }

    fn saturate_check(&self, previous: &Self) -> Result<(), ()> {
        match (&previous.inner, &self.inner) {
            (
                FrameSequenceItemInner::UnsaturatedInput {
                    ..
                }
                | FrameSequenceItemInner::UnsaturatedScheduled
                | FrameSequenceItemInner::Saturating {
                    ..
                },
                _,
            )
            | (
                _,
                FrameSequenceItemInner::Saturating {
                    ..
                }
                | FrameSequenceItemInner::SaturatedInput {
                    ..
                }
                | FrameSequenceItemInner::SaturatedScheduled {
                    ..
                },
            ) => Err(()),
            _ => Ok(()),
        }
    }

    // SAFETY: must be run after saturate_check
    unsafe fn saturate_from_frame(&mut self, frame: Box<Frame<T>>) {
        take_mut::take(&mut self.inner, |next| {
            let update = match next {
                FrameSequenceItemInner::UnsaturatedInput {
                    inputs,
                } => FrameUpdate::re_update_input(frame, inputs, self.time.clone()).map_err(
                    |(inputs, _)| FrameSequenceItemInner::UnsaturatedInput {
                        inputs,
                    },
                ),
                FrameSequenceItemInner::UnsaturatedScheduled => {
                    FrameUpdate::re_update_scheduled(frame, self.time.clone())
                        .map_err(|_| FrameSequenceItemInner::UnsaturatedScheduled)
                },
                _ => unreachable!(),
            };
            match update {
                Ok(update) => FrameSequenceItemInner::Saturating {
                    update: Box::pin(update),
                },
                Err(old_frame) => old_frame,
            }
        });
    }
}

pub enum FrameSequenceItemInnerPoll<T: Transposer> {
    Pending,
    NeedsState,
    Ready(Vec<T::Output>),
}
