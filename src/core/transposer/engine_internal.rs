use std::collections::VecDeque;
use std::{collections::BTreeMap, task::Context};

use std::cmp::Ordering;
use std::sync::{Arc, RwLock};

use crate::{
    core::event::{Event, RollbackPayload},
    core::schedule_stream::SchedulePoll,
};

use super::{
    transposer::Transposer, transposer_frame::TransposerFrame,
    transposer_function_wrappers::init_events, transposer_update::TransposerUpdate,
};

use super::{transposer_history::TransposerHistory, InputEvent, InternalOutputEvent, OutputEvent};

pub(super) type InputBuffer<T> = BTreeMap<<T as Transposer>::Time, Vec<<T as Transposer>::Input>>;
pub(super) type InputStreamItem<T> = InputEvent<T>;

pub(super) struct TransposerEngineInternal<'a, T: Transposer + 'a> {
    history: TransposerHistory<T>,
    current_transposer_frame: Arc<RwLock<TransposerFrame<T>>>,
    input_buffer: InputBuffer<T>,
    output_buffer: VecDeque<InternalOutputEvent<T>>,
    needs_rollback: Option<T::Time>,
    current_update: TransposerUpdate<'a, T>,
}

impl<'a, T: Transposer + 'a> TransposerEngineInternal<'a, T> {
    pub async fn new(transposer: T) -> TransposerEngineInternal<'a, T> {
        let result = init_events(transposer).await;
        let output_buffer = VecDeque::from(result.output_events);

        TransposerEngineInternal {
            current_transposer_frame: Arc::new(RwLock::new(result.initial_frame.clone())),
            history: TransposerHistory::new(result.initial_frame),
            input_buffer: BTreeMap::new(),
            output_buffer,
            needs_rollback: None,
            current_update: TransposerUpdate::None,
        }
    }

    pub fn try_stage_update(&mut self) {
        // // exit if we already have a staged update.
        if self.current_update.is_some() {
            return;
        }
        let frame_arc = self.current_transposer_frame.clone();
        let mut current_frame = self.current_transposer_frame.write().unwrap();

        let next_inputs = self.input_buffer.first_key_value();
        let next_scheduled = current_frame.schedule.get_min();

        self.current_update = match (next_inputs, next_scheduled) {
            (None, None) => TransposerUpdate::None,
            (Some(_), None) => {
                let (time, inputs) = self.input_buffer.pop_first().unwrap();
                TransposerUpdate::new_input(frame_arc, time, inputs)
            }
            (None, Some(_)) => {
                let next_scheduled = current_frame.schedule.remove_min().unwrap();
                TransposerUpdate::new_schedule(frame_arc, next_scheduled)
            }
            (Some(i), Some(s)) => match i.0.cmp(&s.time) {
                Ordering::Less | Ordering::Equal => {
                    let (time, inputs) = self.input_buffer.pop_first().unwrap();
                    TransposerUpdate::new_input(frame_arc, time, inputs)
                }
                Ordering::Greater => {
                    let next_scheduled = current_frame.schedule.remove_min().unwrap();
                    TransposerUpdate::new_schedule(frame_arc, next_scheduled)
                }
            },
        };
    }

    pub fn unstage_update(&mut self) {
        if let Some((time, mut events)) = self.current_update.unstage() {
            match self.input_buffer.get_mut(&time) {
                Some(vec) => {
                    vec.append(&mut events);
                }
                None => {
                    self.input_buffer.insert(time, events);
                }
            }
        }
    }

    fn prepare_for_insert(&mut self, time: T::Time) {
        // unstage if there is an update for at time or after
        // that update is now invalid.
        if let Some(current_update_time) = self.current_update.time() {
            if current_update_time >= time {
                self.unstage_update();
            }
        }
        let mut needs_rollback = false;
        let current = self.current_transposer_frame.read().unwrap();
        let current_time = current.time;
        std::mem::drop(current);

        if current_time >= time {
            let (frame, input_groups, events_emitted) = self.history.revert(time);
            let mut current = self.current_transposer_frame.write().unwrap();
            *current = frame;
            for (time, inputs) in input_groups {
                self.input_buffer.insert(time, inputs);
            }
            needs_rollback = events_emitted;
        }
        if needs_rollback {
            self.needs_rollback = Some(time);
        }
    }

    pub fn insert(&mut self, event: InputStreamItem<T>) {
        let Event { timestamp, payload } = event;

        self.prepare_for_insert(timestamp);
        match payload {
            RollbackPayload::Payload(payload) => {
                match self.input_buffer.get_mut(&timestamp) {
                    Some(vec) => vec.push(payload),
                    None => {
                        self.input_buffer.insert(timestamp, vec![payload]);
                    }
                };
            }
            RollbackPayload::Rollback => {
                // prepare for insert has ensured that all events after timestamp are in the input buffer.
                while let Some((time, inputs)) = self.input_buffer.pop_first() {
                    if time < timestamp {
                        self.input_buffer.insert(time, inputs);
                        break;
                    }
                }
            }
        }
    }

    pub fn poll(
        &mut self,
        time: T::Time,
        cx: &mut Context<'_>,
    ) -> SchedulePoll<T::Time, OutputEvent<T>> {
        loop {
            self.try_stage_update();
            if let Some(timestamp) = self.needs_rollback {
                self.needs_rollback = None;
                let payload = RollbackPayload::Rollback;
                let event = Event { timestamp, payload };
                break SchedulePoll::Ready(event);
            }
            if let Some(event) = self.output_buffer.pop_front() {
                let Event { timestamp, payload } = event;
                let payload = RollbackPayload::Payload(payload);
                let event = Event { timestamp, payload };
                break SchedulePoll::Ready(event);
            }

            match self.current_update.poll(time, cx) {
                SchedulePoll::Ready((result, time, inputs)) => {
                    // push history
                    self.history
                        .push_events(time, inputs, !result.output_events.is_empty());
                    self.history.push_frame(result.new_frame.clone());

                    // write to current_transposer_frame.
                    *self.current_transposer_frame.write().unwrap() = result.new_frame;

                    // push output events.
                    for output in result.output_events {
                        self.output_buffer.push_back(output);
                    }
                }
                SchedulePoll::Pending => break SchedulePoll::Pending,
                SchedulePoll::Scheduled(t) => break SchedulePoll::Scheduled(t),
                SchedulePoll::Done => break SchedulePoll::Done,
            }
        }
    }

    pub fn size_hint(&self) -> (usize, Option<usize>) {
        match self.current_transposer_frame.try_read() {
            Ok(frame) => {
                let min = frame.schedule.len() - frame.expire_handles.len();
                (min, None)
            }
            Err(_) => (0, None),
        }
    }
}
