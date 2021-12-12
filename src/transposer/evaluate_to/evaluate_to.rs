use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::Future;

use crate::transposer::input_buffer::InputBuffer;
use crate::transposer::step_group::{InterpolatePoll, StepGroup, StepGroupPollResult};
use crate::transposer::Transposer;
use crate::util::take_mut;

pub fn evaluate_to<T: Transposer, S, Fs>(
    transposer: T,
    until: T::Time,
    events: Vec<(T::Time, T::Input)>,
    state: S,
    seed: [u8; 32],
) -> EvaluateTo<T, S, Fs>
where
    S: Fn(T::Time) -> Fs,
    Fs: Future<Output = T::InputState>,
{
    let mut input_buffer = InputBuffer::<T::Time, T::Input>::new();
    for (time, input) in events {
        if time <= until && T::can_handle(time, &input) {
            input_buffer.insert_back(time, input);
        }
    }
    EvaluateTo {
        inner: EvaluateToInner::Step {
            frame: StepGroup::new_init(transposer, seed),
            events: input_buffer,
            state,
            state_fut: None,
            until,
            outputs: Vec::new(),
        },
    }
}

pub struct EvaluateTo<T: Transposer, S, Fs>
where
    S: Fn(T::Time) -> Fs,
    Fs: Future<Output = T::InputState>,
{
    inner: EvaluateToInner<T, S, Fs>,
}

enum EvaluateToInner<T: Transposer, S, Fs>
where
    S: Fn(T::Time) -> Fs,
    Fs: Future<Output = T::InputState>,
{
    Step {
        frame:     StepGroup<T>,
        events:    InputBuffer<T::Time, T::Input>,
        state:     S,
        state_fut: Option<Fs>,
        until:     T::Time,
        outputs:   EmittedEvents<T>,
    },
    Interpolate {
        frame:     StepGroup<T>,
        state:     S,
        state_fut: Option<Fs>,
        until:     T::Time,
        outputs:   EmittedEvents<T>,
    },
    Terminated,
}

pub type EmittedEvents<T> = Vec<(<T as Transposer>::Time, Vec<<T as Transposer>::Output>)>;

impl<T: Transposer, S, Fs> Future for EvaluateTo<T, S, Fs>
where
    S: Clone + Fn(T::Time) -> Fs,
    Fs: Future<Output = T::InputState>,
{
    type Output = (EmittedEvents<T>, T::OutputState);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };

        loop {
            let poll_result = take_mut::take_and_return_or_recover(
                &mut this.inner,
                || EvaluateToInner::Terminated,
                |inner| {
                    match inner {
                        EvaluateToInner::Step {
                            mut frame,
                            mut events,
                            state,
                            mut state_fut,
                            until,
                            mut outputs,
                        } => {
                            if let Some(fut) = &mut state_fut {
                                let fut = unsafe { Pin::new_unchecked(fut) };
                                match fut.poll(cx) {
                                    Poll::Ready(s) => {
                                        let _ = frame.set_input_state(s);
                                        state_fut = None;
                                    },
                                    Poll::Pending => {
                                        return (
                                            EvaluateToInner::Step {
                                                frame,
                                                events,
                                                state,
                                                state_fut,
                                                until,
                                                outputs,
                                            },
                                            Poll::Pending,
                                        )
                                    },
                                }
                            }
                            let time = frame.raw_time();
                            let progress = frame.poll_progress(0, cx.waker().clone()).unwrap();
                            outputs.push((time, progress.outputs));
                            match progress.result {
                                StepGroupPollResult::NeedsState => {
                                    state_fut = Some((state)(time));
                                    return (
                                        EvaluateToInner::Step {
                                            frame,
                                            events,
                                            state,
                                            state_fut,
                                            until,
                                            outputs,
                                        },
                                        Poll::Ready(None),
                                    )
                                },
                                StepGroupPollResult::Pending => {
                                    return (
                                        EvaluateToInner::Step {
                                            frame,
                                            events,
                                            state,
                                            state_fut,
                                            until,
                                            outputs,
                                        },
                                        Poll::Pending,
                                    )
                                },
                                StepGroupPollResult::Ready => {},
                            }

                            let mut event = events.pop_first();

                            if let Some(mut next_frame) =
                                frame.next_unsaturated(&mut event).unwrap()
                            {
                                if next_frame.raw_time() <= until {
                                    if let Some((time, inputs)) = event {
                                        events.extend_front(time, inputs);
                                    }
                                    next_frame.saturate_take(&mut frame).unwrap();
                                    frame = next_frame;
                                    return (
                                        EvaluateToInner::Step {
                                            frame,
                                            events,
                                            state,
                                            state_fut,
                                            until,
                                            outputs,
                                        },
                                        Poll::Ready(None),
                                    )
                                }
                            };

                            // no updates or updates after "until"

                            (
                                EvaluateToInner::Interpolate {
                                    frame,
                                    state,
                                    state_fut: None,
                                    until,
                                    outputs,
                                },
                                Poll::Ready(None),
                            )
                        },
                        EvaluateToInner::Interpolate {
                            mut frame,
                            state,
                            mut state_fut,
                            until,
                            outputs,
                        } => {
                            if let Some(fut) = &mut state_fut {
                                let fut = unsafe { Pin::new_unchecked(fut) };
                                match fut.poll(cx) {
                                    Poll::Ready(s) => {
                                        let _ = frame.set_interpolation_input_state(until, 0, s);
                                        state_fut = None;
                                    },
                                    Poll::Pending => {
                                        return (
                                            EvaluateToInner::Interpolate {
                                                frame,
                                                state,
                                                state_fut,
                                                until,
                                                outputs,
                                            },
                                            Poll::Pending,
                                        )
                                    },
                                }
                            }

                            let poll = frame
                                .poll_interpolate(until, 0, cx.waker().clone())
                                .unwrap();

                            match poll {
                                InterpolatePoll::NeedsState => {
                                    state_fut = Some((state)(until));
                                    (
                                        EvaluateToInner::Interpolate {
                                            frame,
                                            state,
                                            state_fut,
                                            until,
                                            outputs,
                                        },
                                        Poll::Ready(None),
                                    )
                                },
                                InterpolatePoll::Pending => (
                                    EvaluateToInner::Interpolate {
                                        frame,
                                        state,
                                        state_fut,
                                        until,
                                        outputs,
                                    },
                                    Poll::Pending,
                                ),
                                InterpolatePoll::Ready(s) => {
                                    (EvaluateToInner::Terminated, Poll::Ready(Some((outputs, s))))
                                },
                            }
                        },
                        EvaluateToInner::Terminated => unreachable!(),
                    }
                },
            );

            match poll_result {
                Poll::Ready(Some(s)) => break Poll::Ready(s),
                Poll::Ready(None) => continue,
                Poll::Pending => break Poll::Pending,
            }
        }
    }
}
