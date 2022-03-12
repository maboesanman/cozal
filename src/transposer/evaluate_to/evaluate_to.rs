use core::hash::Hash;
use core::pin::Pin;
use core::task::{Context, Poll};
use std::collections::VecDeque;
use std::sync::Arc;

use futures_core::Future;

use crate::transposer::schedule_storage::StorageFamily;
use crate::transposer::step::{Interpolation, Step, StepPollResult};
use crate::transposer::Transposer;
use crate::util::replace_mut;

#[derive(Clone, Copy)]
struct Storage;

impl StorageFamily for Storage {
    type OrdMap<K: Ord + Eq + Clone, V: Clone> = std::collections::BTreeMap<K, V>;
    type HashMap<K: Hash + Eq + Clone, V: Clone> = std::collections::HashMap<K, V>;
    type Transposer<T: Clone> = Arc<T>;
    type LazyState<T> = Arc<T>;
}

pub fn evaluate_to<T: Transposer, S, Fs>(
    transposer: T,
    until: T::Time,
    mut events: Vec<(T::Time, T::Input)>,
    state: S,
    seed: [u8; 32],
) -> EvaluateTo<T, S, Fs>
where
    S: Unpin + Fn(T::Time) -> Fs,
    Fs: Future<Output = T::InputState>,
{
    let mut collected_events = VecDeque::new();
    events.sort_by_key(|(t, _)| *t);
    if let Some((mut current_time, _)) = events.first() {
        let mut e = Vec::new();
        for (time, input) in events {
            if time > until {
                break
            }
            if time > current_time {
                collected_events
                    .push_back((current_time, core::mem::take(&mut e).into_boxed_slice()));

                current_time = time;
            }
            e.push(input)
        }
        if !e.is_empty() {
            collected_events.push_back((current_time, e.into_boxed_slice()));
        }
    }
    EvaluateTo {
        inner: EvaluateToInner::Step {
            frame: Box::new(Step::new_init(transposer, seed)),
            events: collected_events,
            state,
            state_fut: None,
            until,
            outputs: Vec::new(),
        },
    }
}

pub struct EvaluateTo<T: Transposer, S, Fs>
where
    S: Unpin + Fn(T::Time) -> Fs,
    Fs: Future<Output = T::InputState>,
{
    inner: EvaluateToInner<T, S, Fs>,
}

type EventGroup<T> = (<T as Transposer>::Time, Box<[<T as Transposer>::Input]>);

enum EvaluateToInner<T: Transposer, S, Fs>
where
    S: Unpin + Fn(T::Time) -> Fs,
    Fs: Future<Output = T::InputState>,
{
    Step {
        frame:     Box<Step<T, Storage>>,
        events:    VecDeque<EventGroup<T>>,
        state:     S,
        state_fut: Option<Pin<Box<Fs>>>,
        until:     T::Time,
        outputs:   EmittedEvents<T>,
    },
    Interpolate {
        interpolate: Pin<Box<Interpolation<T, Storage>>>,
        state:       S,
        state_fut:   Option<Pin<Box<Fs>>>,
        until:       T::Time,
        outputs:     EmittedEvents<T>,
    },
    Terminated,
}

pub type EmittedEvents<T> = Vec<(<T as Transposer>::Time, Vec<<T as Transposer>::Output>)>;

impl<T: Transposer, S, Fs> Future for EvaluateTo<T, S, Fs>
where
    S: Clone + Unpin + Fn(T::Time) -> Fs,
    Fs: Future<Output = T::InputState>,
    T::Output: Unpin,
{
    type Output = (EmittedEvents<T>, T::OutputState);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        loop {
            let poll_result = replace_mut::replace_and_return(
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
                                let fut = Pin::new(fut);
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
                                    state_fut = Some(Box::pin((state)(time)));
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
                                let mut event = events.pop_front();
                                let next = frame.next_unsaturated(&mut event).unwrap();
                                if let Some((time, inputs)) = event {
                                    events.push_front((time, inputs));
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

                            let interpolate = Box::pin(frame.interpolate(until).unwrap());

                            (
                                EvaluateToInner::Interpolate {
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
                            mut interpolate,
                            state,
                            mut state_fut,
                            until,
                            outputs,
                        } => {
                            if let Some(fut) = &mut state_fut {
                                let fut = Pin::new(fut);
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
                                        state_fut = Some(Box::pin((state)(until)));
                                        Poll::Ready(None)
                                    } else {
                                        Poll::Pending
                                    };
                                    (
                                        EvaluateToInner::Interpolate {
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
