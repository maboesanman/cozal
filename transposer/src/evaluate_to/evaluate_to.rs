use core::future::Future;
use core::hash::Hash;
use core::pin::Pin;
use core::task::{Context, Poll};
use std::collections::VecDeque;
use std::mem::replace;
use std::sync::Arc;

use futures_util::FutureExt;

use crate::schedule_storage::StorageFamily;
use crate::step::{Interpolation, Step, StepPoll};
use crate::Transposer;

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
        frame: Box::new(Step::new_init(transposer, seed)),
        interpolate: None,
        events: collected_events,
        state,
        state_fut: None,
        until,
        outputs: Vec::new(),
    }
}

pub struct EvaluateTo<T: Transposer, S, Fs>
where
    S: Unpin + Fn(T::Time) -> Fs,
    Fs: Future<Output = T::InputState>,
{
    frame:       Box<Step<T, Storage>>,
    interpolate: Option<Box<Interpolation<T, Storage>>>,
    events:      VecDeque<EventGroup<T>>,
    state:       S,
    state_fut:   Option<Pin<Box<Fs>>>,
    until:       T::Time,
    outputs:     EmittedEvents<T>,
}

type EventGroup<T> = (<T as Transposer>::Time, Box<[<T as Transposer>::Input]>);

pub type EmittedEvents<T> = Vec<(<T as Transposer>::Time, <T as Transposer>::Output)>;

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
            // deal with pending state futures
            if let Some(state_fut) = &mut this.state_fut {
                let state = state_fut.poll_unpin(cx);
                let state = match state {
                    Poll::Ready(s) => {
                        this.state_fut = None;
                        s
                    },
                    Poll::Pending => return Poll::Pending,
                };
                if let Some(interpolate) = &mut this.interpolate {
                    if interpolate.set_state(state).is_err() {
                        panic!()
                    }
                } else {
                    if this.frame.set_input_state(state).is_err() {
                        panic!()
                    }
                }
            }

            // deal with the current interpolation
            if let Some(interpolate) = &mut this.interpolate {
                let result = interpolate.poll_unpin(cx);
                let result = match result {
                    Poll::Ready(r) => {
                        this.interpolate = None;
                        r
                    },
                    Poll::Pending => {
                        if interpolate.needs_state() {
                            let fut = (this.state)(this.until);
                            this.state_fut = Some(Box::pin(fut));
                            continue
                        } else {
                            return Poll::Pending
                        }
                    },
                };
                let outputs = replace(&mut this.outputs, Vec::new());
                return Poll::Ready((outputs, result))
            }

            // deal with the step.
            let poll = this.frame.poll(cx.waker().clone()).unwrap();
            match poll {
                StepPoll::NeedsState => {
                    let fut = (this.state)(this.frame.raw_time());
                    this.state_fut = Some(Box::pin(fut));
                    continue
                },
                StepPoll::Emitted(e) => {
                    this.outputs.push((this.frame.raw_time(), e));
                    continue
                },
                StepPoll::Pending => return Poll::Pending,
                StepPoll::Ready => {
                    let mut next_event = this.events.pop_front();
                    let mut next_frame = this.frame.next_unsaturated(&mut next_event).unwrap();
                    if let Some(next_event) = next_event {
                        this.events.push_front(next_event)
                    };

                    if let Some(f) = &next_frame {
                        if f.raw_time() > this.until {
                            next_frame = None
                        }
                    }

                    match next_frame {
                        Some(mut f) => {
                            f.saturate_take(&mut this.frame).unwrap();
                            this.frame = Box::new(f);
                            continue
                        },
                        None => {
                            this.interpolate =
                                Some(Box::new(this.frame.interpolate(this.until).unwrap()));
                            continue
                        },
                    };
                },
            }
        }
    }
}
