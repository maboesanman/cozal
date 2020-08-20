use super::transposer::Transposer;
use super::transposer_context::TransposerContext;
use super::transposer_event::{
    ExternalTransposerEvent, InitialTransposerEvent, InternalTransposerEvent, TransposerEvent,
};
use crate::core::event::event::{Event, EventTimestamp};
use core::pin::Pin;
use core::sync::atomic::Ordering::Relaxed;
use futures::{
    stream::Enumerate,
    task::{Context, Poll, Waker},
    StreamExt,
};
use futures::{Future, Stream};
use im::{HashMap, OrdSet};
use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, VecDeque};
use std::sync::{Arc, Weak};

#[derive(Clone)]
pub(super) struct TransposerFrame<T: Transposer> {
    // this is an Rc because we might not change the transposer, and therefore don't need to save a copy.
    transposer: Arc<T>,

    //schedule and expire_handles
    schedule: OrdSet<Arc<TransposerEvent<T>>>,
    expire_handles: HashMap<u64, Weak<TransposerEvent<T>>>,
    current_expire_handle: u64,
    // constants for the current randomizer
}

enum NextEvent<T: Transposer> {
    None,
    Internal(Arc<TransposerEvent<T>>),
    External(Arc<TransposerEvent<T>>),
}

pub(super) struct TransposerEngineInternal<
    'a,
    T: Transposer + 'a,
    S: Stream<Item = Event<T::External>> + Unpin + Send + 'a,
> {
    input_stream: Enumerate<S>,

    // modify to be a historical collection thing
    transposer_frame: TransposerFrame<T>,

    // this is a min heap of events and indexes, sorted first by event, then by index.
    input_buffer: BinaryHeap<Reverse<(Event<T::External>, usize)>>,
    output_buffer: VecDeque<Event<T::Out>>,
    pub current_update:
        Option<Pin<Box<dyn Future<Output = (TransposerFrame<T>, Vec<Event<T::Out>>)> + Send + 'a>>>,
    pub current_waker: Option<Waker>,
}

impl<'a, T: Transposer + 'a, S: Stream<Item = Event<T::External>> + Unpin + Send + 'a>
    TransposerEngineInternal<'a, T, S>
{
    pub(super) async fn new(input_stream: S) -> TransposerEngineInternal<'a, T, S> {
        let (transposer_frame, output_buffer) = Self::init().await;
        TransposerEngineInternal {
            input_stream: input_stream.enumerate(),
            transposer_frame,
            input_buffer: BinaryHeap::new(),
            output_buffer: VecDeque::from(output_buffer),
            current_update: None,
            current_waker: None,
        }
    }

    pub(super) fn poll(
        &mut self,
        cx: &mut Context<'_>,
        until: &EventTimestamp,
    ) -> Poll<Option<Event<T::Out>>> {
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

    fn poll_input(&mut self, cx: &mut Context<'_>, _until: &EventTimestamp) {
        let poll_result = Pin::new(&mut self.input_stream).poll_next(cx);
        if let Poll::Ready(Some((index, event))) = poll_result {
            self.input_buffer.push(Reverse((event, index)));
        }
        // this is where we put the rollback code.
    }

    fn poll_update(
        &mut self,
        cx: &mut Context<'_>,
        until: &EventTimestamp,
    ) -> Option<Poll<Vec<Event<T::Out>>>> {
        // set current_update if it is not already set.
        if let None = self.current_update {
            if let Some((frame, fut)) = self.get_next_update(until) {
                self.current_update = Some(fut);
                self.transposer_frame = frame;
            }
        };
        // poll current_update, setting it to none if ready.
        if let Some(fut) = &mut self.current_update {
            Some(match Pin::new(fut).poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready((frame, events)) => {
                    self.current_update = None;
                    self.transposer_frame = frame;
                    Poll::Ready(events)
                }
            })
        } else {
            None
        }
    }

    async fn init() -> (TransposerFrame<T>, Vec<Event<T::Out>>) {
        let cx = TransposerContext::new(0);
        let result = T::init(&cx).await;

        let mut new_events = Vec::new();
        for (index, event) in result.new_events.into_iter().enumerate() {
            let event = Arc::new(event);
            let init_event = InitialTransposerEvent { index, event };
            let transposer_event = TransposerEvent::Initial(init_event);
            new_events.push(Arc::new(transposer_event));
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

        (
            TransposerFrame {
                transposer: Arc::new(result.new_updater),
                schedule,
                expire_handles,
                current_expire_handle: cx.current_expire_handle.load(Relaxed),
            },
            result.emitted_events,
        )
    }

    async fn update(
        frame: TransposerFrame<T>,
        parent: Arc<TransposerEvent<T>>,
    ) -> (TransposerFrame<T>, Vec<Event<T::Out>>) {
        let cx = TransposerContext::new(frame.current_expire_handle);
        let trigger = parent.into_trigger_event();
        let result = frame.transposer.update(&cx, &trigger).await;

        let mut new_events = Vec::new();
        for (index, event) in result.new_events.into_iter().enumerate() {
            let event = Arc::new(event);
            let init_event = InternalTransposerEvent {
                parent: parent.clone(),
                index,
                event,
            };
            let transposer_event = TransposerEvent::Internal(init_event);
            new_events.push(Arc::new(transposer_event));
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

        // add events to emission
        let output_buffer = Vec::from(result.emitted_events);

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
            output_buffer,
        )
    }

    fn get_next_update(
        &self,
        _until: &EventTimestamp,
    ) -> Option<(
        TransposerFrame<T>,
        Pin<Box<dyn Future<Output = (TransposerFrame<T>, Vec<Event<T::Out>>)> + Send + 'a>>,
    )> {
        let next_external = self.input_buffer.peek();
        let next_external = match next_external {
            None => None,
            Some(event) => {
                let Reverse((event, index)) = event.clone();
                let event = Arc::new(event);
                let event = ExternalTransposerEvent::<T> { index, event };
                Some(TransposerEvent::External(event))
            }
        };
        let (next_internal, new_schedule) = self.transposer_frame.schedule.without_min();

        let next_event = match (next_external, next_internal) {
            (None, None) => NextEvent::None,
            (Some(e), None) => NextEvent::External(Arc::new(e)),
            (None, Some(e)) => NextEvent::Internal(e),
            (Some(ext), Some(int)) => match ext.cmp(int.as_ref()) {
                Ordering::Less => NextEvent::External(Arc::new(ext)),
                Ordering::Equal => panic!(),
                Ordering::Greater => NextEvent::Internal(int),
            },
        };
        match next_event {
            NextEvent::None => None,
            NextEvent::Internal(next_event) => {
                // use the schedule with the event removed if we are using the internal event
                let mut new_frame = self.transposer_frame.clone();
                new_frame.schedule = new_schedule;
                let fut = Self::update(new_frame.clone(), next_event);
                let fut = Box::pin(fut);
                Some((new_frame, fut))
            }
            NextEvent::External(next_event) => {
                // do not use the schedule with the event removed if we are not using the internal event
                let new_frame = self.transposer_frame.clone();
                let fut = Self::update(new_frame.clone(), next_event);
                let fut = Box::pin(fut);
                Some((new_frame, fut))
            }
        }
    }
}
