use super::{
    transposer::Transposer,
    transposer_context::TransposerContext,
    transposer_event::{InternalTransposerEvent, TransposerEvent},
    transposer_frame::TransposerFrame,
};
use crate::core::event::event::Event;
use im::{HashMap, OrdSet};
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;

pub(super) type WrappedInitResult<T> = (
    TransposerFrame<T>,
    Vec<Event<<T as Transposer>::Time, <T as Transposer>::Out>>,
);

// wrapper for transposer init
pub(super) async fn init<T: Transposer>() -> WrappedInitResult<T> {
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

    let emitted_events: Vec<_> = result
        .emitted_events
        .into_iter()
        .map(|payload| Event {
            timestamp: T::Time::default(),
            payload,
        })
        .collect::<Vec<_>>();

    (
        TransposerFrame {
            transposer: Arc::new(result.new_updater),
            schedule,
            expire_handles,
            current_expire_handle: cx.current_expire_handle.load(SeqCst),
        },
        emitted_events,
    )
}

pub(super) type WrappedUpdateResult<T> = (
    TransposerFrame<T>,
    Vec<Event<<T as Transposer>::Time, <T as Transposer>::Out>>,
    bool,
);

// wrapper for transposer update
pub(super) async fn update<T: Transposer>(
    frame: TransposerFrame<T>,
    events: Vec<TransposerEvent<T>>, // these are assumed to be at the same time and sorted.
) -> WrappedUpdateResult<T> {
    let cx = TransposerContext::new(frame.current_expire_handle);
    let events_refs: Vec<&TransposerEvent<T>> = events.iter().collect();
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

    let emitted_events: Vec<_> = result
        .emitted_events
        .into_iter()
        .map(|payload| Event { timestamp, payload })
        .collect();

    let transposer = match result.new_updater {
        Some(u) => Arc::new(u),
        None => frame.transposer.clone(),
    };

    (
        TransposerFrame {
            transposer,
            schedule,
            expire_handles,
            current_expire_handle: cx.current_expire_handle.load(SeqCst),
        },
        emitted_events,
        result.exit,
    )
}
