use std::collections::{HashMap, VecDeque};
use std::pin::Pin;
use std::task::Poll;

use super::output_buffer::OutputBuffer;
use super::storage::{DummySendStorage, TransposeStorage};
use super::transpose_metadata::TransposeMetadata;
use crate::source::source_poll::SourcePollOk;
use crate::source::SourcePoll;
use crate::transposer::input_buffer::InputBuffer;
use crate::transposer::step::{Interpolation, Step};
use crate::transposer::Transposer;

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
    last_time: T::Time,
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

    pub fn buffer_output_events(&mut self, time: T::Time, outputs: Vec<T::Output>) {
        for val in outputs.into_iter() {
            self.output_buffer.handle_event(time, val);
        }
    }

    pub fn poll_output_buffer(&mut self, time: T::Time) -> SourcePollOk<T::Time, T::Output, ()> {
        self.output_buffer.poll(time)
    }

    /// Correct our step list to respect the "working-poll-next" invariant
    ///
    /// what is the "working-poll-next" invariant?????
    /// our steps always need to be in one of the following formats:
    ///
    /// - Saturating Step
    /// - Unsaturated Step (any number, including 0)
    /// - Poll Time
    /// - Step (any number, including 0)
    ///
    /// or
    ///
    /// - Saturated Step (with empty schedule)
    /// - Poll Time
    /// - Step (any number, including 0)
    ///
    /// or
    ///
    /// - Saturated Step (with non-empty schedule, next scheduled after poll time)
    /// - Poll Time
    /// - Step (any positive number)
    ///
    /// Ensuring this invariant includes deciding which steps to clone and which to take when saturating.
    pub fn ensure_working_poll_next_invariant(&mut self, time: T::Time) {
        todo!()
    }

    pub fn prepare_for_interpolation(
        &mut self,
        channel: usize,
        time: T::Time,
    ) -> Pin<&mut Interpolation<T, TransposeStorage>> {
        todo!()
    }

    pub fn clear_channel(&mut self, channel: usize) {
        todo!()
    }

    pub fn handle_source_poll<S, Err>(
        &mut self,
        poll: SourcePoll<T::Time, T::Input, S, Err>,
    ) -> PollResult<S> {
        match poll {
            Poll::Pending => PollResult::Pending,
            Poll::Ready(Err(_)) => panic!(),
            Poll::Ready(Ok(SourcePollOk::Event(e, t))) => {
                self.handle_input_event(t, e);
                PollResult::Continue
            },
            Poll::Ready(Ok(SourcePollOk::Rollback(t))) => {
                self.handle_input_rollback(t);
                PollResult::Continue
            },
            Poll::Ready(Ok(SourcePollOk::Finalize(t))) => {
                self.handle_input_finalize(t);
                PollResult::Continue
            },
            Poll::Ready(Ok(SourcePollOk::Scheduled(s, t))) => {
                self.handle_scheduled(t);
                PollResult::Ready(s)
            },
            Poll::Ready(Ok(SourcePollOk::Ready(s))) => PollResult::Ready(s),
        }
    }
}

pub enum PollResult<T> {
    Ready(T),
    Pending,
    Continue,
}
