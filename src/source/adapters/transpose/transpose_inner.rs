use std::collections::{HashMap, VecDeque};
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use futures_core::Future;

use super::output_buffer::OutputBuffer;
use super::storage::{DummySendStorage, TransposeStorage};
use super::transpose_metadata::TransposeMetadata;
use crate::source::source_poll::{SourcePollErr, SourcePollOk};
use crate::source::traits::SourceContext;
use crate::source::SourcePoll;
use crate::transposer::input_buffer::InputBuffer;
use crate::transposer::step::{Interpolation, Metadata, Step, StepPoll, StepPollResult};
use crate::transposer::Transposer;
use crate::util::stack_waker::StackWaker;
use crate::util::vecdeque_helpers;

pub struct TransposeInner<T: Transposer> {
    steps:             Steps<T>,
    pending_channels:  HashMap<usize, ChannelData<T>>,
    current_scheduled: Option<T::Time>,
    input_finalized:   Option<T::Time>,

    output_buffer: OutputBuffer<T>,
    input_buffer:  InputBuffer<T>,
}

// a collection of Rc which are guranteed not to be cloned outside the collection is Send
// whenever the same collection, but with Arc would be Send, so we do an unsafe impl for exactly that situation.
unsafe impl<T: Transposer> Send for Steps<T> where Step<T, DummySendStorage, TransposeMetadata>: Send
{}
struct Steps<T: Transposer>(VecDeque<StepWrapper<T>>);
struct StepWrapper<T: Transposer> {
    step:           Step<T, TransposeStorage, TransposeMetadata>,
    events_emitted: bool,
}

impl<T: Transposer> StepWrapper<T> {
    pub fn new_init(transposer: T, rng_seed: [u8; 32]) -> Self {
        Self {
            step:           Step::new_init(transposer, rng_seed),
            events_emitted: false,
        }
    }
}

struct ChannelData<T: Transposer> {
    last_time:     T::Time,
    interpolation: Interpolation<T, TransposeStorage>,
}

/// the result of poll
///
/// After returning NeedsState the source should be polled, and the callback called if a state is available
pub enum InnerPoll<'a, T: Transposer, S, Err> {
    Output {
        time:   T::Time,
        output: T::Output,
    },
    Pending,
    NeedsState {
        time: T::Time,
        channel: usize,
        one_channel_waker: Waker,
        forget: bool,
        handle_source_poll_callback: HandleSourcePollCallback<'a, T, S, Err>,
    },
    Ready(S),
    Scheduled(S, T::Time),
}

pub type HandleSourcePollCallback<'a, T, S, Err> = Box<
    dyn FnOnce(
            SourcePoll<
                <T as Transposer>::Time,
                <T as Transposer>::Input,
                <T as Transposer>::InputState,
                Err,
            >,
        ) -> Result<
            HandleSourcePollCallbackResult<'a, T, S, Err>,
            SourcePollErr<<T as Transposer>::Time, Err>,
        > + 'a,
>;

pub enum HandleSourcePollCallbackResult<'a, T: Transposer, S, Err> {
    Pending,
    PollAgain(HandleSourcePollCallback<'a, T, S, Err>),
    Ready(Result<InnerPoll<'a, T, S, Err>, SourcePollErr<T::Time, Err>>),
}

impl<T: Transposer> TransposeInner<T> {
    pub fn new(transposer: T, rng_seed: [u8; 32]) -> Self {
        let mut steps = VecDeque::new();
        steps.push_back(StepWrapper::new_init(transposer, rng_seed));
        let steps = Steps(steps);
        Self {
            steps,
            pending_channels: HashMap::new(),
            current_scheduled: None,
            input_finalized: None,
            output_buffer: OutputBuffer::new(),
            input_buffer: InputBuffer::new(),
        }
    }

    /// gets the step we need to work on. this should be the first saturating or saturated before or at time.
    ///
    /// returns a mutable reference to the step.
    pub fn get_working_step(
        &mut self,
        time: T::Time,
    ) -> &mut Step<T, TransposeStorage, TransposeMetadata> {
        // step[i] is first step for which step.raw_time() > time.
        // step[i - 1] is the last step for which step.raw_time() <= time.
        let i = self.steps.0.partition_point(|s| s.step.raw_time() <= time);

        if i < 1 {
            panic!("polled time before T::Time::default")
        }

        let mut i = i - 1;
        let steps = &mut self.steps;
        loop {
            let step = steps.0.get_mut(i).map(|s| &mut s.step).unwrap();
            if !step.is_unsaturated() {
                break
            }
            i -= 1;
        }
        steps.0.get_mut(i).map(|s| &mut s.step).unwrap()
    }

    /// this is the first possible time to emit an event, barring any new inputs
    fn get_first_original_step_time(&self) -> Option<T::Time> {
        let step = &self.steps.0.back()?.step;

        if step.is_original() {
            Some(step.raw_time())
        } else {
            None
        }
    }

    /// return the earliest time between the source schedule, first original, and first_output_buffer
    pub fn get_scheduled_time(&self) -> Option<T::Time> {
        let current_scheduled = self.current_scheduled;
        let first_original = self.get_first_original_step_time();
        let first_output_buffer = self.output_buffer.first_event_time();

        let mut result = current_scheduled;
        result = match (result, first_original) {
            (None, t) => t,
            (t, None) => t,
            (Some(t1), Some(t2)) => Some(t1.min(t2)),
        };
        result = match (result, first_output_buffer) {
            (None, t) => t,
            (t, None) => t,
            (Some(t1), Some(t2)) => Some(t1.min(t2)),
        };

        return result
    }

    fn handle_input_event(&mut self, time: T::Time, event: T::Input) {
        self.current_scheduled = None;
        self.input_buffer.insert_back(time, event);
        // delete active interpolations, clear source channels, wake wakers
        todo!()
    }
    fn handle_input_rollback(&mut self, time: T::Time) {
        self.current_scheduled = None;
        self.input_buffer.rollback(time);
        todo!()
    }
    fn handle_input_finalize(&mut self, time: T::Time) {
        self.current_scheduled = None;
        self.input_finalized = Some(time);
        todo!()
    }
    fn handle_scheduled(&mut self, time: T::Time) {
        self.current_scheduled = Some(time);
        todo!()
    }
    pub fn handle_caller_advance(&mut self, time: T::Time) {
        // throw away pending channels that were last polled before time
        todo!()
    }

    /// returns list of source channels to release
    pub fn handle_caller_release_channel(&mut self, channel: usize) -> Vec<usize> {
        todo!()
    }

    pub fn poll_output_buffer(&mut self, time: T::Time) -> SourcePollOk<T::Time, T::Output, ()> {
        self.output_buffer.poll(time)
    }

    pub fn poll<Err>(
        &mut self,
        poll_time: T::Time,
        forget: bool,
        cx: SourceContext,
    ) -> Result<InnerPoll<'_, T, T::OutputState, Err>, SourcePollErr<T::Time, Err>> {
        // use an existing channel if it exists
        if let Some(channel_data) = self.pending_channels.get_mut(&cx.channel) {
            if channel_data.last_time != poll_time {
                // the existing channel must be discarded because the time doesn't match
                self.pending_channels.remove(&cx.channel);
            } else {
                let waker = cx.one_channel_waker.clone();
                let mut context = Context::from_waker(&waker);
                match Pin::new(&mut channel_data.interpolation).poll(&mut context) {
                    Poll::Ready(output_state) => {
                        drop(channel_data);
                        self.pending_channels.remove(&cx.channel);

                        return Ok(match self.get_scheduled_time() {
                            Some(t) => InnerPoll::Scheduled(output_state, t),
                            None => InnerPoll::Ready(output_state),
                        })
                    },
                    Poll::Pending => {
                        if channel_data.interpolation.needs_state() {
                            let waker = cx.one_channel_waker.clone();

                            // THIS IS NOT REQUIRED WITH POLONIUS
                            // SAFETY: because we are returning immediately, we know that the mutable reference to self is not aliased.
                            let channel_data: *mut _ = channel_data;
                            let channel_data: &'_ mut _ = unsafe { &mut *channel_data };

                            // THIS MUT BORROWS SELF (via channel_data)
                            return Ok(InnerPoll::NeedsState {
                                time: poll_time,
                                channel: cx.channel * 2 + 1,
                                forget,
                                one_channel_waker: cx.one_channel_waker.clone(),
                                handle_source_poll_callback: Box::new(
                                    move |source_poll: SourcePoll<
                                        T::Time,
                                        T::Input,
                                        T::InputState,
                                        Err,
                                    >| {
                                        let source_poll = match source_poll {
                                            Poll::Ready(source_poll) => source_poll,
                                            Poll::Pending => {
                                                return Ok(HandleSourcePollCallbackResult::Pending)
                                            },
                                        };

                                        match source_poll? {
                                            SourcePollOk::Rollback(t) => {
                                                self.handle_input_rollback(t);
                                            },
                                            SourcePollOk::Event(e, t) => {
                                                self.handle_input_event(t, e);
                                            },
                                            SourcePollOk::Finalize(t) => {
                                                self.handle_input_finalize(t);
                                            },
                                            SourcePollOk::Scheduled(s, t) => {
                                                self.current_scheduled = Some(t);
                                                let _ =
                                                    channel_data.interpolation.set_state(s, &waker);
                                            },
                                            SourcePollOk::Ready(s) => {
                                                let _ =
                                                    channel_data.interpolation.set_state(s, &waker);
                                            },
                                        };

                                        Ok(HandleSourcePollCallbackResult::Ready(
                                            self.poll(poll_time, forget, cx),
                                        ))
                                    },
                                ),
                            })
                        }
                    },
                }
            }
        }

        // step[i] is first step for which step.raw_time() > time.
        // step[i - 1] is the last step for which step.raw_time() <= time.
        let i = self
            .steps
            .0
            .partition_point(|s| s.step.raw_time() <= poll_time);

        if i < 1 {
            return Err(SourcePollErr::PollBeforeDefault)
        }

        self.poll_steps(poll_time, forget, cx, i - 1)
    }

    fn poll_steps<Err>(
        &mut self,
        poll_time: T::Time,
        forget: bool,
        cx: SourceContext,
        start_i: usize,
    ) -> Result<InnerPoll<'_, T, T::OutputState, Err>, SourcePollErr<T::Time, Err>> {
        // walk i backwards, until step[i] is saturating or saturated.
        let mut i = start_i;
        let steps = &mut self.steps;
        loop {
            let step = &mut steps.0.get_mut(i).unwrap().step;
            match step.get_metadata_mut() {
                Metadata::Unsaturated(()) => {
                    i -= 1;
                    continue
                },
                Metadata::Saturating(metadata) => {
                    // retrieve the waker, only if we actually need to poll.
                    let waker = match StackWaker::register(
                        &mut metadata.stack_waker,
                        cx.channel,
                        cx.one_channel_waker.clone(),
                    ) {
                        Some(w) => w,
                        None => break Ok(InnerPoll::Pending),
                    };

                    let StepPoll {
                        result,
                        outputs,
                    } = step.poll(waker.clone()).unwrap();

                    let mut outputs = outputs.into_iter();

                    if let Some(output) = outputs.next() {
                        for val in outputs {
                            self.output_buffer.handle_event(step.raw_time(), val);
                        }
                        break Ok(InnerPoll::Output {
                            time: step.raw_time(),
                            output,
                        })
                    }

                    match result {
                        StepPollResult::NeedsState => {
                            // THIS IS NOT REQUIRED WITH POLONIUS
                            // SAFETY: because we are returning immediately, we know that the mutable reference to self is not aliased.
                            let step: *mut _ = step;
                            let step: &'_ mut _ = unsafe { &mut *step };
                            // THIS MUT BORROWS SELF (via step)
                            return Ok(InnerPoll::NeedsState {
                                time: poll_time,
                                channel: cx.channel * 2 + 1,
                                forget: false,
                                one_channel_waker: waker.clone(),
                                handle_source_poll_callback: Box::new(
                                    move |source_poll: SourcePoll<
                                        T::Time,
                                        T::Input,
                                        T::InputState,
                                        Err,
                                    >| {
                                        let source_poll = match source_poll {
                                            Poll::Ready(source_poll) => source_poll,
                                            Poll::Pending => {
                                                return Ok(HandleSourcePollCallbackResult::Pending)
                                            },
                                        };

                                        match source_poll? {
                                            SourcePollOk::Rollback(t) => {
                                                self.handle_input_rollback(t);
                                            },
                                            SourcePollOk::Event(e, t) => {
                                                self.handle_input_event(t, e);
                                            },
                                            SourcePollOk::Finalize(t) => {
                                                self.handle_input_finalize(t);
                                            },
                                            SourcePollOk::Scheduled(s, t) => {
                                                self.current_scheduled = Some(t);
                                                let _ = step.set_input_state(s, &waker);
                                            },
                                            SourcePollOk::Ready(s) => {
                                                let _ = step.set_input_state(s, &waker);
                                            },
                                        };

                                        Ok(HandleSourcePollCallbackResult::Ready(
                                            self.poll_steps(poll_time, forget, cx, i),
                                        ))
                                    },
                                ),
                            })
                        },
                        StepPollResult::Pending => break Ok(InnerPoll::Pending),
                        StepPollResult::Ready => {
                            i -= 1;
                            continue
                        },
                    }
                },
                Metadata::Saturated(()) => {
                    let (step, next) =
                        vecdeque_helpers::get_with_next_mut(&mut steps.0, i).unwrap();

                    // if there's another step we need to saturate then do that one first.
                    if let Some(next) = next {
                        if next.step.raw_time() <= poll_time {
                            next.step.saturate_clone(&step.step).unwrap();
                            i += 1;
                            continue
                        }
                    }

                    let interpolation = step.step.interpolate(poll_time).unwrap();
                    let channel_data = ChannelData {
                        last_time: poll_time,
                        interpolation,
                    };

                    self.pending_channels.insert(cx.channel, channel_data);
                    break self.poll(poll_time, forget, cx)
                },
            }
        }
    }

    pub fn clear_channel(&mut self, channel: usize) {
        todo!()
    }

    pub fn handle_source_poll<S, Err>(
        &mut self,
        poll: SourcePoll<T::Time, T::Input, S, Err>,
    ) -> Result<PollResult<S>, SourcePollErr<T::Time, Err>> {
        Ok(match poll {
            Poll::Pending => PollResult::Pending,
            Poll::Ready(Err(e)) => return Err(e),
            Poll::Ready(Ok(SourcePollOk::Event(e, t))) => {
                self.handle_input_event(t, e);
                PollResult::PollAgain
            },
            Poll::Ready(Ok(SourcePollOk::Rollback(t))) => {
                self.handle_input_rollback(t);
                PollResult::PollAgain
            },
            Poll::Ready(Ok(SourcePollOk::Finalize(t))) => {
                self.handle_input_finalize(t);
                PollResult::PollAgain
            },
            Poll::Ready(Ok(SourcePollOk::Scheduled(s, t))) => {
                self.handle_scheduled(t);
                PollResult::Ready(s)
            },
            Poll::Ready(Ok(SourcePollOk::Ready(s))) => PollResult::Ready(s),
        })
    }
}

pub enum PollResult<T> {
    Ready(T),
    Pending,
    PollAgain,
}
