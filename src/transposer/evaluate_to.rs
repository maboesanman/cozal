use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::Future;

use super::context::{InputStateContext, InterpolateContext};
use super::lazy_state::LazyState;
use super::step::Step;
use super::{step, Transposer};
use crate::source::adapters::transpose::input_buffer::InputBuffer;

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
        if time <= until {
            input_buffer.insert_back(time, input);
        }
    }
    EvaluateTo {
        inner: EvaluateToInner::Step {
            frame: Step::new_init(transposer, seed),
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
    Pending {
        transposer: T,
        until:      T::Time,
        events:     Vec<(T::Time, T::Input)>,
        state:      S,
        seed:       [u8; 32],
    },
    Step {
        frame:     Step<'static, T>,
        events:    InputBuffer<T::Time, T::Input>,
        state:     S,
        state_fut: Option<Fs>,
        until:     T::Time,
        outputs:   Vec<(T::Time, Vec<T::Output>)>,
    },
    Interpolate {
        future:     MaybeUninit<Pin<Box<dyn Future<Output = T::OutputState>>>>,
        context:    MyInterpolateContext<T, S, Fs>,
        transposer: T,
        outputs:    Vec<(T::Time, Vec<T::Output>)>,
    },
}

impl<T: Transposer, S, Fs> Drop for EvaluateToInner<T, S, Fs>
where
    S: Fn(T::Time) -> Fs,
    Fs: Future<Output = T::InputState>,
{
    fn drop(&mut self) {
        if let EvaluateToInner::Interpolate {
            future, ..
        } = self
        {
            unsafe { future.assume_init_drop() };
        }
    }
}

impl<T: Transposer, S, Fs> Future for EvaluateTo<T, S, Fs>
where
    S: Clone + Fn(T::Time) -> Fs,
    Fs: Future<Output = T::InputState>,
{
    type Output = (Vec<(T::Time, Vec<T::Output>)>, T::OutputState);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        loop {
            match &mut this.inner {
                EvaluateToInner::Step {
                    frame,
                    events,
                    state,
                    state_fut,
                    until,
                    outputs,
                } => {
                    if let Some(fut) = state_fut {
                        let fut = unsafe { Pin::new_unchecked(fut) };
                        match fut.poll(cx) {
                            Poll::Ready(s) => {
                                let _ = frame.set_input_state(s);
                                *state_fut = None;
                            },
                            Poll::Pending => break Poll::Pending,
                        }
                    }
                    let until = *until;
                    let time = frame.time().raw_time();
                    match frame.poll(cx.waker().clone()).unwrap() {
                        step::StepPoll::Pending => break Poll::Pending,
                        step::StepPoll::NeedsState => {
                            *state_fut = Some((state)(time));
                            continue
                        },
                        step::StepPoll::ReadyNoOutputs => {},
                        step::StepPoll::ReadyOutputs(o) => {
                            outputs.push((time, o));
                        },
                    }

                    let mut event = events.pop_first();

                    match frame.next_unsaturated(&mut event).unwrap() {
                        Some(mut next_frame) => {
                            if next_frame.time().raw_time() <= until {
                                for (time, inputs) in event {
                                    events.extend_front(time, inputs);
                                }
                                next_frame.saturate_take(frame).unwrap();
                                *frame = next_frame;
                                continue
                            }
                        },
                        None => {},
                    };

                    // no updates or updates after "until"

                    let base_time = frame.time().raw_time();

                    this.inner = EvaluateToInner::Interpolate {
                        future:     MaybeUninit::uninit(),
                        context:    MyInterpolateContext::new(until, state.clone()),
                        transposer: frame.desaturate().unwrap().unwrap(),
                        outputs:    core::mem::replace(outputs, Vec::new()),
                    };

                    if let EvaluateToInner::Interpolate {
                        future,
                        context,
                        transposer,
                        ..
                    } = &mut this.inner
                    {
                        let fut = transposer.interpolate(base_time, until, context);
                        *future = MaybeUninit::new(unsafe { core::mem::transmute(fut) });
                    }
                },
                EvaluateToInner::Interpolate {
                    future,
                    outputs,
                    ..
                } => {
                    break match unsafe { future.assume_init_mut() }.as_mut().poll(cx) {
                        Poll::Ready(output_state) => {
                            Poll::Ready((core::mem::replace(outputs, Vec::new()), output_state))
                        },
                        Poll::Pending => Poll::Pending,
                    }
                },
            }
        }
    }
}

struct MyInterpolateContext<T: Transposer, S, Fs>
where
    S: Fn(T::Time) -> Fs,
    Fs: Future<Output = T::InputState>,
{
    time:     T::Time,
    state_fn: S,
    state:    LazyState<T::InputState>,
    t:        PhantomData<fn() -> T>,
}

impl<T: Transposer, S, Fs> MyInterpolateContext<T, S, Fs>
where
    S: Fn(T::Time) -> Fs,
    Fs: Future<Output = T::InputState>,
{
    pub fn new(time: T::Time, state_fn: S) -> Self {
        Self {
            time,
            state_fn,
            state: LazyState::new(),
            t: PhantomData,
        }
    }

    pub fn reset(&mut self) {
        self.state = LazyState::default();
    }
}

impl<'a, T: Transposer, S, Fs> InterpolateContext<'a, T> for MyInterpolateContext<T, S, Fs>
where
    S: Fn(T::Time) -> Fs,
    Fs: Future<Output = T::InputState>,
{
}

impl<'a, T: Transposer, S, Fs> InputStateContext<'a, T> for MyInterpolateContext<T, S, Fs>
where
    S: Fn(T::Time) -> Fs,
    Fs: Future<Output = T::InputState>,
{
    fn get_input_state(&mut self) -> Pin<Box<dyn 'a + Future<Output = &'a T::InputState>>> {
        let fut = async {
            if self.state.requested() {
                let state_fut = (self.state_fn)(self.time);
                let _ = self.state.set(state_fut.await);
            }

            let s: *const _ = &self.state;
            let s: &'a _ = unsafe { s.as_ref::<'a>().unwrap() };
            s.await
        };

        let b: Pin<Box<dyn '_ + Future<Output = &'a T::InputState>>> = Box::pin(fut);

        unsafe { core::mem::transmute(b) }
    }
}
