use futures::task::{Context, Poll};
use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, VecDeque};
use std::sync::Arc;

use crate::{
    core::event::event::{Event, RollbackPayload},
    core::schedule_stream::schedule_stream::SchedulePoll,
    utilities::full_ord::{full_cmp, FullOrd},
};

use super::{
    transposer::Transposer,
    transposer_event::{ExternalTransposerEvent, TransposerEvent},
    transposer_frame::TransposerFrame,
    transposer_function_wrappers::{init, update},
    transposer_update::TransposerUpdate,
};

pub(super) type InputBuffer<T> = BinaryHeap<Reverse<FullOrd<ExternalTransposerEvent<T>>>>;
pub(super) type InputStreamItem<'a, T> =
    Event<<T as Transposer>::Time, RollbackPayload<<T as Transposer>::External>>;

pub(super) struct HistoryFrame<T: Transposer> {
    time: T::Time,
    events: Vec<TransposerEvent<T>>,
    frame: TransposerFrame<T>,
}

pub(super) struct TransposerEngineInternal<'a, T: Transposer + 'a> {
    initial_frame: TransposerFrame<T>,
    history: Vec<HistoryFrame<T>>,
    input_buffer: InputBuffer<T>,
    output_buffer: VecDeque<Event<T::Time, T::Out>>,
    current_update: Option<TransposerUpdate<'a, T>>,
}

impl<'a, T: Transposer + 'a> TransposerEngineInternal<'a, T> {
    pub async fn new() -> TransposerEngineInternal<'a, T> {
        let (initial_frame, output_buffer) = init::<T>().await;
        TransposerEngineInternal {
            initial_frame,
            history: Vec::new(),
            input_buffer: BinaryHeap::new(),
            output_buffer: VecDeque::from(output_buffer),
            current_update: None,
        }
    }

    pub fn try_stage_update(&mut self) {
        // exit if we already have a staged update.
        if self.current_update.is_some() {
            return;
        }

        let current_frame = match self.history.last() {
            None => &self.initial_frame,
            Some(history_frame) => &history_frame.frame,
        };

        let mut new_frame = current_frame.clone();
        let mut events: Vec<TransposerEvent<T>> = Vec::new();
        let mut time: Option<T::Time> = None;

        loop {
            let next_external = self.input_buffer.peek();
            let next_external = match next_external {
                Some(Reverse(FullOrd(e))) => Some(TransposerEvent::External(e.clone())),
                None => None,
            };

            let (next_internal, schedule_without_next_internal) = new_frame.schedule.without_min();
            let next_internal = match next_internal {
                Some(e) => Some(TransposerEvent::Internal(e.as_ref().clone())),
                None => None,
            };

            let next_event: TransposerEvent<T> = match (next_external, next_internal) {
                (None, None) => break,
                (Some(e), None) => e,
                (None, Some(e)) => e,
                (Some(ext), Some(int)) => match full_cmp(&ext, &int) {
                    Ordering::Less => ext,
                    Ordering::Equal => panic!(),
                    Ordering::Greater => int,
                },
            };

            if let Some(time) = time {
                if time != next_event.timestamp() {
                    break;
                }
            } else {
                time = Some(next_event.timestamp());
            }

            match next_event {
                TransposerEvent::External(_) => {
                    self.input_buffer.pop();
                }
                TransposerEvent::Internal(_) => {
                    new_frame.schedule = schedule_without_next_internal;
                }
            }
            events.push(next_event);
        }

        self.current_update = if let Some(time) = time {
            let future = update(new_frame, events.clone());
            let future = Box::pin(future);
            let result = Poll::Pending;
            Some(TransposerUpdate {
                time,
                events,
                future,
                result,
            })
        } else {
            None
        }
    }

    pub fn unstage_update(&mut self) {
        let staged_update = std::mem::take(&mut self.current_update);

        if let Some(staged_update) = staged_update {
            for transposer_event in staged_update.events {
                match transposer_event {
                    TransposerEvent::External(external_event) => {
                        self.input_buffer.push(Reverse(FullOrd(external_event)));
                    }
                    TransposerEvent::Internal(_) => {
                        // we don't need to do anything here because this lives in the history.
                    }
                }
            }
        }
    }

    pub fn try_commit_update(
        &mut self,
        time: T::Time,
        cx: &mut Context<'_>,
    ) -> SchedulePoll<T::Time, Vec<Event<T::Time, T::Out>>> {
        // let staged_update = std::mem::take(self.current_update);

        if let Some(staged_update) = &mut self.current_update {
            staged_update.poll(cx);

            if time < staged_update.time {
                return SchedulePoll::Scheduled(staged_update.time);
            }

            if let Poll::Pending = staged_update.result {
                return SchedulePoll::Pending;
            }

            let staged_update = std::mem::take(&mut self.current_update);
            let staged_update = staged_update.unwrap();
            let (frame, out_events, exit) = match staged_update.result {
                Poll::Ready(r) => r,
                _ => unreachable!(),
            };
            if exit {
                return SchedulePoll::Done;
            }
            let new_history_frame = HistoryFrame {
                time: staged_update.time,
                events: staged_update.events,
                frame,
            };
            self.history.push(new_history_frame);

            SchedulePoll::Ready(out_events)
        } else {
            SchedulePoll::Pending
        }
    }

    fn revert(&mut self) {
        if let Some(historyFrame) = self.history.pop() {
            for transposer_event in historyFrame.events {
                match transposer_event {
                    TransposerEvent::External(external_event) => {
                        self.input_buffer.push(Reverse(FullOrd(external_event)));
                    }
                    TransposerEvent::Internal(_) => {
                        // we don't need to do anything here because this lives in the history.
                    }
                }
            }
        }
    }

    fn prepare_for_insert(&mut self, time: T::Time) {
        // unstage if there is an update for at time or after
        // that update is now invalid.
        if let Some(update) = &self.current_update {
            if update.time >= time {
                self.unstage_update();
            }
        }
        while let Some(history_frame) = self.history.last() {
            if history_frame.time >= time {
                self.revert();
            } else {
                break;
            }
        }
    }

    /// scrub all events from all sources which occur at or after `time`
    ///
    /// this must be run after prepare_for_insert.
    fn rollback(&mut self, time: T::Time) {
        let mut new_input_buffer: InputBuffer<T> = BinaryHeap::new();

        while let Some(Reverse(FullOrd(event))) = self.input_buffer.pop() {
            if event.event.timestamp >= time {
                break;
            }
            new_input_buffer.push(Reverse(FullOrd(event)));
        }
        self.input_buffer = new_input_buffer;
    }

    pub fn insert(&mut self, event: InputStreamItem<T>) {
        let Event { timestamp, payload } = event;

        self.prepare_for_insert(timestamp);
        match payload {
            RollbackPayload::Payload(payload) => {
                let event = Event { timestamp, payload };
                let event = Arc::new(event);
                let event = ExternalTransposerEvent::<T> { event };
                let event = Reverse(FullOrd(event));
                self.input_buffer.push(event);
            }
            RollbackPayload::Rollback => {
                // prepare for insert has ensured that all events after timestamp are in the input buffer.
                self.rollback(timestamp);
            }
        }
    }

    pub fn poll(
        &mut self,
        time: T::Time,
        cx: &mut Context<'_>,
    ) -> SchedulePoll<T::Time, Event<T::Time, RollbackPayload<T::Out>>> {
        self.try_stage_update();
        loop {
            if let Some(event) = self.output_buffer.pop_front() {
                let Event { timestamp, payload } = event;
                let payload = RollbackPayload::Payload(payload);
                let event = Event { timestamp, payload };
                break SchedulePoll::Ready(event);
            }
            match self.try_commit_update(time, cx) {
                SchedulePoll::Ready(events) => {
                    for event in events.into_iter() {
                        self.output_buffer.push_back(event);
                    }
                }
                SchedulePoll::Pending => break SchedulePoll::Pending,
                SchedulePoll::Scheduled(t) => break SchedulePoll::Scheduled(t),
                SchedulePoll::Done => break SchedulePoll::Done,
            }
        }
    }
}
