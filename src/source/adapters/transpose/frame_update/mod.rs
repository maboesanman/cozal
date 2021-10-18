use core::future::Future;
use core::mem::MaybeUninit;
use core::pin::Pin;
use core::task::{Context, Poll};

use futures_core::FusedFuture;

use self::frame_update_pollable::FrameUpdatePollable;

// use self::frame_update_pollable::FrameUpdatePollable;
use super::engine_time::{EngineTime, EngineTimeSchedule};
use super::transposer_frame::TransposerFrame;
use super::update_result::UpdateResult;
use crate::transposer::Transposer;

mod frame_update_pollable;

/// future to initialize a TransposerFrame
///
/// this type owns a frame, and has many self references.
pub struct FrameUpdate<T: Transposer>(FrameUpdateInner<T>);

enum FrameUpdateInner<T: Transposer> {
    // Unpollable variants hold the references until we can create the pollable future
    Unpollable(FrameUpdateUnpollable<T>),
    Pollable(FrameUpdatePollable<T>),
    Terminated,
}

enum UpdateType<T: Transposer> {
    Init,
    Input {
        time:   T::Time,
        inputs: Vec<T::Input>,
    },
    Scheduled {
        time:    EngineTimeSchedule<T::Time>,
        payload: T::Scheduled,
    },
}

struct FrameUpdateUnpollable<T: Transposer> {
    frame: TransposerFrame<T>,
    args: UpdateType<T>,
}

pub enum NextFrameUpdate<T: Transposer> {
    None {
        frame: TransposerFrame<T>
    },
    Input {
        frame_update: FrameUpdate<T>,
    },
    Scheduled {
        frame_update: FrameUpdate<T>,
        time: EngineTimeSchedule<T::Time>,
        unused_inputs: Option<Vec<T::Input>>,
    }
}

impl<T: Transposer> FrameUpdate<T>
where
    T::Scheduled: Clone,
{
    #[allow(unused)]
    pub fn new_init(
        frame: TransposerFrame<T>,
    ) -> Self {
        Self(FrameUpdateInner::Unpollable(FrameUpdateUnpollable {
            frame,
            args: UpdateType::Init,
        }))
    }

    /// start an update
    ///
    /// the optional time return is:
    ///
    /// Some(t) if the new update is a scheduled update occurring at time t, where t < time
    ///
    /// None if the new update is an input update using the inputs.
    pub fn next_update(
        frame: TransposerFrame<T>,
        time: T::Time,
        mut inputs: Option<Vec<T::Input>>,
    ) -> NextFrameUpdate<T> {
        let mut next_scheduled_time = frame.get_next_scheduled_time();
        if let Some(nst) = next_scheduled_time {
            if time < nst.time {
                next_scheduled_time = None
            } else if time == nst.time && inputs.is_some() {
                next_scheduled_time = None
            }
        }

        match (inputs, next_scheduled_time) {
            (None, None) => NextFrameUpdate::None { frame },
            (Some(inputs), None) => NextFrameUpdate::Input {
                frame_update: Self::new_input(frame, time, inputs)
            },
            (_, Some(_)) => {
                todo!()
                // frame.pop_schedule_event();
                // NextFrameUpdate::Scheduled {
                //     frame_update: Self::new_scheduled(frame, time, payload),
                //     time: ets
                //     unused_inputs: inputs,
                // }
            },
        }
    }

    // pub fn time(&self) -> Result<EngineTime<T::Time>, ()> {
    //     match self.0 {
    //         FrameUpdateInner::Unpollable(unpollable) => Ok(match unpollable.args {
    //             UpdateType::Init => EngineTime::Init,
    //             UpdateType::Input { time, .. } => EngineTime::Input(time),
    //             UpdateType::Scheduled { time, .. } => EngineTime::Schedule(time),
    //         }),
    //         FrameUpdateInner::Pollable(_) => todo!(),
    //         FrameUpdateInner::Terminated => todo!(),
    //     }
    // }

    fn new_input(
        frame: TransposerFrame<T>,
        time: T::Time,
        inputs: Vec<T::Input>,
    ) -> Self {
        Self(FrameUpdateInner::Unpollable(FrameUpdateUnpollable {
            frame,
            args: UpdateType::Input {
                time,
                inputs,
            },
        }))
    }

    fn new_scheduled(
        frame: TransposerFrame<T>,
        time: EngineTimeSchedule<T::Time>,
        payload: T::Scheduled,
    ) -> Self {
        Self(FrameUpdateInner::Unpollable(FrameUpdateUnpollable {
            frame,
            args: UpdateType::Scheduled {
                time,
                payload,
            },
        }))
    }
}

impl<T: Transposer> Future for FrameUpdate<T> {
    type Output = UpdateResult<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        // SAFETY the pin is only relavent for the pollable variant, which we don't move around.
        let this = unsafe { self.get_unchecked_mut() };
        let inner = &mut this.0;
        'poll: loop {
            match inner {
                FrameUpdateInner::Unpollable(_) => {
                    // get vars ready to take out of self.
                    let mut args: MaybeUninit<UpdateType<T>> = MaybeUninit::uninit();

                    // take out of self and replace with a Pollable with uninit future.
                    take_mut::take(
                        inner,
                        |inner_owned| {
                            if let FrameUpdateInner::Unpollable(FrameUpdateUnpollable {
                                frame,
                                args: args_temp,
                            }) = inner_owned
                            {
                                args = MaybeUninit::new(args_temp);

                                // SAFETY: we are calling init in just a few lines
                                FrameUpdateInner::Pollable(unsafe { FrameUpdatePollable::new(frame) })
                            } else {
                                unreachable!()
                            }
                        },
                    );

                    // init out uninit context and future.
                    if let FrameUpdateInner::Pollable(pollable) = inner {
                        // SAFETY: these must have been assigned if we are now pollable.
                        let args = unsafe { args.assume_init() };
                        let pollable = unsafe { Pin::new_unchecked(pollable) };

                        // SAFETY: new must have been called if we are now pollable
                        pollable.init(args)
                    } else {
                        unreachable!()
                    }
                },
                FrameUpdateInner::Pollable(pollable) => {
                    // SAFETY: structural pinning. we don't move the future if we don't move self.
                    let pollable = unsafe { Pin::new_unchecked(pollable) };

                    // pass through the poll
                    break 'poll match pollable.poll(cx) {
                        Poll::Ready(()) => {
                            if let FrameUpdateInner::Pollable(pollable) = core::mem::replace(inner, FrameUpdateInner::Terminated) {
                                let (frame, outputs) = pollable.reclaim();

                                Poll::Ready(UpdateResult {
                                    frame,
                                    outputs,
                                    exit: false,
                                })
                            } else {
                                unreachable!()
                            }
                        },
                        Poll::Pending => Poll::Pending,
                    }
                },
                FrameUpdateInner::Terminated => break 'poll Poll::Pending,
            };
        }
    }
}

impl<T: Transposer> FusedFuture for FrameUpdate<T> {
    fn is_terminated(&self) -> bool {
        matches!(self.0, FrameUpdateInner::Terminated)
    }
}
