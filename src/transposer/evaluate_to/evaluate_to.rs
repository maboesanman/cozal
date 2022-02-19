use std::hash::Hash;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

use futures_core::Future;

use crate::transposer::input_buffer::InputBuffer;
use crate::transposer::schedule_storage::StorageFamily;
use crate::transposer::step::{PointerInterpolation, Step, StepPollResult};
use crate::transposer::Transposer;
use crate::util::take_mut;

#[derive(Clone, Copy)]
struct Storage;

impl StorageFamily for Storage {
    type OrdMap<K: Ord + Eq + Clone, V: Clone> = std::collections::BTreeMap<K, V>;
    type HashMap<K: Hash + Eq + Clone, V: Clone> = std::collections::HashMap<K, V>;
    type Transposer<T: Clone> = Box<T>;
}

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
    let mut input_buffer = InputBuffer::<T>::new();
    for (time, input) in events {
        if time <= until && T::can_handle(time, &input) {
            input_buffer.insert_back(time, input);
        }
    }
    EvaluateTo {
        inner: EvaluateToInner::Step {
            frame: Box::new(Step::new_init(transposer, seed)),
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
        frame:     Box<Step<T, Storage>>,
        events:    InputBuffer<T>,
        state:     S,
        state_fut: Option<Fs>,
        until:     T::Time,
        outputs:   EmittedEvents<T>,
    },
    Interpolate {
        frame:       Box<Step<T, Storage>>,
        interpolate: Pin<Box<PointerInterpolation<T>>>,
        state:       S,
        state_fut:   Option<Fs>,
        until:       T::Time,
        outputs:     EmittedEvents<T>,
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
                                        let _ = frame.set_input_state(s, cx.waker());
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
                            let progress = frame.poll(cx.waker().clone()).unwrap();
                            outputs.push((time, progress.outputs));
                            match progress.result {
                                StepPollResult::NeedsState {
                                    ..
                                } => {
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
                                StepPollResult::Pending {
                                    ..
                                } => {
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
                                StepPollResult::Ready {
                                    ..
                                } => {},
                            }

                            let next = {
                                let mut event = events.pop_first();
                                let next = frame.next_unsaturated(&mut event).unwrap();
                                if let Some((time, inputs)) = event {
                                    events.extend_front(time, inputs);
                                }

                                next
                            };

                            if let Some(mut next_frame) = next {
                                if next_frame.raw_time() <= until {
                                    next_frame.saturate_take(&mut frame).unwrap();
                                    *frame = next_frame;
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

                            let interpolate = Box::pin(frame.interpolate_pointer(until).unwrap());

                            (
                                EvaluateToInner::Interpolate {
                                    frame,
                                    interpolate,
                                    state,
                                    state_fut: None,
                                    until,
                                    outputs,
                                },
                                Poll::Ready(None),
                            )
                        },
                        EvaluateToInner::Interpolate {
                            frame,
                            mut interpolate,
                            state,
                            mut state_fut,
                            until,
                            outputs,
                        } => {
                            if let Some(fut) = &mut state_fut {
                                let fut = unsafe { Pin::new_unchecked(fut) };
                                match fut.poll(cx) {
                                    Poll::Ready(s) => {
                                        if interpolate.set_state(s, cx.waker()).is_err() {
                                            panic!()
                                        };
                                        state_fut = None;
                                    },
                                    Poll::Pending => {
                                        return (
                                            EvaluateToInner::Interpolate {
                                                frame,
                                                interpolate,
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

                            let poll = interpolate.as_mut().poll(cx);

                            match poll {
                                Poll::Pending => {
                                    let result = if interpolate.needs_state() {
                                        state_fut = Some((state)(until));
                                        Poll::Ready(None)
                                    } else {
                                        Poll::Pending
                                    };
                                    (
                                        EvaluateToInner::Interpolate {
                                            frame,
                                            interpolate,
                                            state,
                                            state_fut,
                                            until,
                                            outputs,
                                        },
                                        result,
                                    )
                                },
                                Poll::Ready(s) => {
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
