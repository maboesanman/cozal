use super::transposer::Transposer;
use super::transposer_context::TransposerContext;
use super::transposer_event::{
    ExternalTransposerEvent, InternalTransposerEvent, TransposerEvent,
};
use crate::{utilities::full_ord::{full_cmp, FullOrd}, core::event::event::{RollbackPayload, Event}};
use core::pin::Pin;
use core::sync::atomic::Ordering::Relaxed;
use futures::{
    task::{Context, Poll, Waker},
};
use futures::{Future, Stream, StreamExt, stream::Fuse};
use im::{HashMap, OrdSet};
use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, VecDeque};
use std::sync::{Arc, Weak};

#[derive(Clone)]
pub(super) struct TransposerFrame<T: Transposer> {// this is an Arc because we might not change the transposer, and therefore don't need to save a copy.
    transposer: Arc<T>,

    // schedule and expire_handles
    schedule: OrdSet<Arc<InternalTransposerEvent<T>>>,
    expire_handles: HashMap<u64, Weak<InternalTransposerEvent<T>>>,

    current_expire_handle: u64,
    // constants for the current randomizer
}

enum NextEvent {
    None,
    Internal,
    External,
}

pub(super) struct TransposerEngineInternal<
    'a,
    T: Transposer + 'a,
    S: Stream<Item = Event<T::Time, RollbackPayload<T::External>>> + Unpin + Send + 'a,
> {
    input_stream: Fuse<S>,

    initial_frame: TransposerFrame<T>,
    history: Vec<(Vec<Arc<TransposerEvent<T>>>, TransposerFrame<T>)>,

    // this is a min heap of events and indexes, sorted first by event, then by index.
    input_buffer: BinaryHeap<Reverse<FullOrd<ExternalTransposerEvent<T>>>>,
    output_buffer: VecDeque<Event<T::Time, T::Out>>,

    pub current_update: Option<(
        T::Time,
        Pin<Box<dyn Future<Output = (TransposerFrame<T>, Vec<Arc<TransposerEvent<T>>>, Vec<Event<T::Time, T::Out>>)> + Send + 'a>>
    )>,
    pub current_waker: Option<Waker>,
}

impl<'a, T: Transposer + 'a, S: Stream<Item = Event<T::Time, RollbackPayload<T::External>>> + Unpin + Send + 'a>
    TransposerEngineInternal<'a, T, S>
{
    pub(super) async fn new(input_stream: S) -> TransposerEngineInternal<'a, T, S> {
        let (initial_frame, output_buffer) = Self::init().await;
        TransposerEngineInternal {
            input_stream: input_stream.fuse(),
            initial_frame,
            history: Vec::new(),
            input_buffer: BinaryHeap::new(),
            output_buffer: VecDeque::from(output_buffer),
            current_update: None,
            current_waker: None,
        }
    }

    pub(super) fn poll(
        &mut self,
        cx: &mut Context<'_>,
        until: &T::Time,
    ) -> Poll<Option<Event<T::Time, T::Out>>> {
        self.poll_input(cx, until);
        loop {
            if let Some(out) = self.output_buffer.pop_front() {
                break Poll::Ready(Some(out));
            }
            match self.poll_update(cx, until) {
                None => break Poll::Pending,
                Some(Poll::Pending) => break Poll::Pending,
                Some(Poll::Ready(events)) => {
                    self.output_buffer = VecDeque::from(events);
                }
            }
        }
    }

    pub(super) fn next_scheduled_time(&self) -> Option<T::Time> {
        match self.current_frame().schedule.get_min() {
            Some(e) => Some(e.event.timestamp),
            None => None,
        }
    }

    fn current_time(&self) -> T::Time {
        match self.history.last() {
            None => T::Time::default(),
            Some((e, _)) => e.first().unwrap().timestamp(),
        }
    }

    fn current_frame(&self) -> &TransposerFrame<T> {
        match self.history.last() {
            None => &self.initial_frame,
            Some((_, frame)) => &frame
        }
    }

    // we move the payload from an event<T, RollbackPayload<P>> into an Event<T, P> in here.
    fn poll_input(&mut self, cx: &mut Context<'_>, until: &T::Time) {
        if self.input_stream.is_done() {
            return;
        }
        let poll_result = Pin::new(&mut self.input_stream).poll_next(cx);
        match poll_result {
            Poll::Ready(Some(Event { payload: RollbackPayload::Payload(payload), timestamp })) => {
                let event = Event {
                    timestamp,
                    payload,
                };
                if T::can_process(&event) {
                    let event = Arc::new(event);
                    let event = ExternalTransposerEvent {
                        event,
                    };
                    self.input_buffer.push(Reverse(FullOrd(event)));
                }
            }
            Poll::Ready(Some(Event { payload: RollbackPayload::Rollback, .. })) => todo!(),
            Poll::Ready(None) => {}
            Poll::Pending => {}
        }
    }

    fn poll_update(
        &mut self,
        cx: &mut Context<'_>,
        until: &T::Time,
    ) -> Option<Poll<Vec<Event<T::Time, T::Out>>>> {
        // set current_update if it is not already set.
        if let None = self.current_update {
            self.current_update = self.get_next_update(until);
        };
        // poll current_update, setting it to none if ready.
        if let Some((_, fut)) = &mut self.current_update {
            Some(match Pin::new(fut).poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready((frame, events, out_events)) => {
                    // TODO don't copy events
                    self.history.push((events.clone(), frame));
                    self.current_update = None;
                    Poll::Ready(out_events)
                }
            })
        } else {
            None
        }
    }

    async fn init() -> (TransposerFrame<T>, Vec<Event<T::Time, T::Out>>) {
        let cx = TransposerContext::new(0);
        let result = T::init(&cx).await;

        let mut new_events = Vec::new();
        for (index, event) in result.new_events.into_iter().enumerate() {
            let event = Arc::new(event);
            let init_event = InternalTransposerEvent {
                created_at: T::Time::default(),
                index, 
                event,
            };
            new_events.push(Arc::new(init_event));
        }

        // add events to schedule
        let mut schedule = OrdSet::new();
        for event in new_events.iter() {
            schedule.insert(event.clone());
        }

        // add expire handles
        let mut expire_handles = HashMap::new();
        for (k, v) in cx.new_expire_handles.lock().unwrap().iter() {
            if let Some(e) = new_events.get(*v) {
                expire_handles.insert(*k, Arc::downgrade(&e.clone()));
            }
        }

        let emitted_events: Vec<_> = result.emitted_events.into_iter().map(|payload| Event {
            timestamp: T::Time::default(),
            payload,
        }).collect::<Vec<_>>();

        (
            TransposerFrame {
                transposer: Arc::new(result.new_updater),
                schedule,
                expire_handles,
                current_expire_handle: cx.current_expire_handle.load(Relaxed),
            },
            emitted_events,
        )
    }

    
    async fn update(
        frame: TransposerFrame<T>,
        events: Vec<Arc<TransposerEvent<T>>>, // these are assumed to be at the same time and sorted.
    ) -> (TransposerFrame<T>, Vec<Arc<TransposerEvent<T>>>, Vec<Event<T::Time, T::Out>>) {
        let cx = TransposerContext::new(frame.current_expire_handle);
        let events_refs: Vec<&TransposerEvent<T>> = events.iter().map(|e| e.as_ref()).collect();
        let timestamp = events_refs.first().unwrap().timestamp();
        let result = frame.transposer.update(&cx, events_refs).await;

        let mut new_events = Vec::new();
        for (index, event) in result.new_events.into_iter().enumerate() {
            let event = Arc::new(event);
            let init_event = InternalTransposerEvent {
                created_at: timestamp,
                index,
                event,
            };
            new_events.push(Arc::new(init_event));
        }

        // add events to schedule
        let mut schedule = frame.schedule.clone();
        for event in new_events.iter() {
            schedule.insert(event.clone());
        }

        // add expire handles
        let mut expire_handles = frame.expire_handles.clone();
        for (k, v) in cx.new_expire_handles.lock().unwrap().iter() {
            if let Some(e) = new_events.get(*v) {
                expire_handles.insert(*k, Arc::downgrade(&e.clone()));
            }
        }

        // remove expired events
        for h in result.expired_events {
            if let Some(e) = frame.expire_handles.get(&h) {
                if let Some(e) = e.upgrade() {
                    schedule.remove(&e);
                    expire_handles.remove(&h);
                }
            }
        }

        let emitted_events: Vec<_> = result.emitted_events.into_iter().map(|payload| Event {
            timestamp,
            payload,
        }).collect();

        let transposer = match result.new_updater {
            Some(u) => Arc::new(u),
            None => frame.transposer.clone(),
        };

        (
            TransposerFrame {
                transposer,
                schedule,
                expire_handles,
                current_expire_handle: cx.current_expire_handle.load(Relaxed),
            },
            events,
            emitted_events,
        )
    }

    fn get_next_update(
        &mut self,
        until: &T::Time,
    ) -> Option<(
        T::Time,
        Pin<Box<dyn Future<Output = (TransposerFrame<T>, Vec<Arc<TransposerEvent<T>>>, Vec<Event<T::Time, T::Out>>)> + Send + 'a>>,
    )> {
        let mut events: Vec<Arc<TransposerEvent<T>>> = Vec::new();
        let mut new_frame = self.current_frame().clone();
        let mut time: Option<T::Time> = None;
        
        loop {
            let next_external = self.input_buffer.peek();
            let next_external = match next_external {
                Some(Reverse(FullOrd(e))) => Some(e.clone()),
                // Some(_) => None,
                None => None,
            };
            let next_external = match next_external {
                Some(e) => Some(TransposerEvent::External(e)),
                // Some(_) => None,
                None => None,
            };
            
            let (next_internal, schedule_without_next_internal) = new_frame.schedule.without_min();
            let next_internal = match next_internal {
                Some(e) => Some(TransposerEvent::Internal(e.as_ref().clone())),
                // Some(_) => None,
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
            events.push(Arc::new(next_event));
        }
        if let Some(time) = time {
            let fut = Self::update(new_frame.clone(), events);
            let fut = Box::pin(fut);
            Some((time, fut))
        } else {
            None
        }
    }
}
