use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::Future;

use super::context::InputStateContext;
use super::sequence_frame::SequenceFrame;
use super::{sequence_frame, Transposer};
use crate::source::adapters::transpose::input_buffer::{self, InputBuffer};

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
        frame: SequenceFrame::new_init(transposer, seed),
        events: input_buffer,
        state,
        until,
        outputs: Vec::new(),
    }
}

pub struct EvaluateTo<T: Transposer, S>
where
    S: Fn(T::Time) -> T::InputState,
{
    interpolation: Option<(Box<T>, Box<dyn Future<Output = T::OutputState>>)>,
    frame:         SequenceFrame<T>,
    events:        InputBuffer<T::Time, T::Input>,
    state:         S,
    until:         T::Time,
    outputs:       Vec<(T::Time, Vec<T::Output>)>,
}

impl<T: Transposer, S> Future for EvaluateTo<T, S>
where
    S: Fn(T::Time) -> T::InputState,
{
    type Output = (Vec<(T::Time, Vec<T::Output>)>, T::OutputState);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        loop {
            if let Some((_, interpolation)) = this.interpolation {
                let interpolation = unsafe { Pin::new_unchecked(interpolation) };
                break match interpolation.as_mut().poll(cx) {
                    Poll::Ready(output_state) => {
                        let outputs = core::mem::replace(&mut this.outputs, Vec::new());
                        Poll::Ready((outputs, output_state))
                    },
                    Poll::Pending => Poll::Pending,
                }
            }
            let time = this.frame.time().raw_time();
            match this.frame.poll(cx.waker().clone()).unwrap() {
                sequence_frame::SequenceFramePoll::Pending => break Poll::Pending,
                sequence_frame::SequenceFramePoll::NeedsState => {
                    let state = (this.state)(time);
                    let _ = this.frame.set_input_state(state);
                    continue
                },
                sequence_frame::SequenceFramePoll::ReadyNoOutputs => {},
                sequence_frame::SequenceFramePoll::ReadyOutputs(outputs) => {
                    this.outputs.push((time, outputs));
                },
            }

            // if time > self.until {

            //     break Poll::Pending
            // }

            let mut event = this.events.pop_first();

            match this.frame.next_unsaturated(&mut event).unwrap() {
                Some(mut next_frame) => {
                    if next_frame.time().raw_time() <= this.until {
                        for (time, inputs) in event {
                            this.events.extend_front(time, inputs);
                        }
                        next_frame.saturate_take(&mut this.frame);
                        this.frame = next_frame;
                        continue
                    }
                },
                None => {},
            };

            let transposer = this.frame.desaturate().unwrap().unwrap();
            let transposer = Box::new(transposer);
            let interpolate_context = InterpolateContext::new(this.until, this.state);
            let interpolation = transposer.interpolate(
                this.frame.time().raw_time(),
                this.until,
                &mut interpolate_context,
            );
        }
    }
}

struct InterpolateContext<T: Transposer, S>
where
    S: Fn(T::Time) -> T::InputState,
{
    time:  T::Time,
    state: S,
    t:     PhantomData<T>,
}

impl<T: Transposer, S> InterpolateContext<T, S>
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

impl<T: Transposer, S> InputStateContext<T> for InterpolateContext<T, S>
where
    S: Fn(T::Time) -> T::InputState,
{
    fn get_input_state(&mut self) -> Pin<&mut dyn Future<Output = T::InputState>> {
        let state = (self.state)(self.time);
        let state: Box<dyn Future<Output = _>> = Box::new(async move { state });

        todo!()
    }
}
