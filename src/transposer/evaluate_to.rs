use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::Future;

use super::context::{InputStateContext, InterpolateContext};
use super::sequence_frame::SequenceFrame;
use super::{sequence_frame, Transposer};
use crate::source::adapters::transpose::input_buffer::InputBuffer;

pub fn evaluate_to<T: Transposer, S>(
    transposer: T,
    until: T::Time,
    events: Vec<(T::Time, T::Input)>,
    state: S,
    seed: [u8; 32],
) -> EvaluateTo<T, S>
where
    S: Fn(T::Time) -> T::InputState,
{
    let mut input_buffer = InputBuffer::<T::Time, T::Input>::new();
    for (time, input) in events {
        if time <= until {
            input_buffer.insert_back(time, input);
        }
    }

    EvaluateTo {
        inner: EvaluateToInner::Step {
            frame: SequenceFrame::new_init(transposer, seed),
            events: input_buffer,
            state,
            until,
            outputs: Vec::new(),
        },
    }
}

pub struct EvaluateTo<T: Transposer, S>
where
    S: Fn(T::Time) -> T::InputState,
{
    inner: EvaluateToInner<T, S>,
}

enum EvaluateToInner<T: Transposer, S>
where
    S: Fn(T::Time) -> T::InputState,
{
    Step {
        frame:   SequenceFrame<T>,
        events:  InputBuffer<T::Time, T::Input>,
        state:   S,
        until:   T::Time,
        outputs: Vec<(T::Time, Vec<T::Output>)>,
    },
    Interpolate {
        future:     MaybeUninit<Pin<Box<dyn Future<Output = T::OutputState>>>>,
        context:    MyInterpolateContext<T, S>,
        transposer: T,
        outputs:    Vec<(T::Time, Vec<T::Output>)>,
    },
}

impl<T: Transposer, S> Drop for EvaluateToInner<T, S>
where
    S: Fn(T::Time) -> T::InputState,
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

impl<T: Transposer, S> Future for EvaluateTo<T, S>
where
    S: Clone + Fn(T::Time) -> T::InputState,
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
                    until,
                    outputs,
                } => {
                    let until = *until;
                    let time = frame.time().raw_time();
                    match frame.poll(cx.waker().clone()).unwrap() {
                        sequence_frame::SequenceFramePoll::Pending => break Poll::Pending,
                        sequence_frame::SequenceFramePoll::NeedsState => {
                            let state = (state)(time);
                            let _ = frame.set_input_state(state);
                            continue
                        },
                        sequence_frame::SequenceFramePoll::ReadyNoOutputs => {},
                        sequence_frame::SequenceFramePoll::ReadyOutputs(o) => {
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

struct MyInterpolateContext<T: Transposer, S>
where
    S: Fn(T::Time) -> T::InputState,
{
    time:  T::Time,
    state: S,
    t:     PhantomData<T>,
}

impl<T: Transposer, S> MyInterpolateContext<T, S>
where
    S: Fn(T::Time) -> T::InputState,
{
    pub fn new(time: T::Time, state: S) -> Self {
        Self {
            time,
            state,
            t: PhantomData,
        }
    }
}

impl<T: Transposer, S> InterpolateContext<T> for MyInterpolateContext<T, S> where
    S: Fn(T::Time) -> T::InputState
{
}

impl<T: Transposer, S> InputStateContext<T> for MyInterpolateContext<T, S>
where
    S: Fn(T::Time) -> T::InputState,
{
    fn get_input_state(&mut self) -> Pin<Box<dyn '_ + Future<Output = T::InputState>>> {
        let state = (self.state)(self.time);
        Box::pin(async move { state })
    }
}
