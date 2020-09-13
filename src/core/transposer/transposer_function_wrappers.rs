use super::{
    context::TransposerContext,
    expire_handle::ExpireHandleFactory,
    internal_scheduled_event::{InternalScheduledEvent, Source},
    transposer_frame::TransposerFrame,
    ExpireHandle, InternalOutputEvent, ScheduledEvent, Transposer, UpdateResult,
};
use crate::core::Event;
use im::{HashMap, OrdSet};
use std::sync::{Arc, RwLock};

pub(super) struct WrappedInitResult<T: Transposer> {
    pub initial_frame: TransposerFrame<T>,
    pub output_events: Vec<InternalOutputEvent<T>>,
}

// wrapper for transposer init
pub(super) async fn init_events<T: Transposer>(transposer: T) -> WrappedInitResult<T> {
    let mut initial_frame = TransposerFrame {
        time: T::Time::default(),
        transposer,
        schedule: OrdSet::new(),
        expire_handles: HashMap::new(),
        expire_handle_factory: ExpireHandleFactory::new(),
    };

    let cx = TransposerContext::new(initial_frame.expire_handle_factory.clone());
    let result = T::init_events(&mut initial_frame.transposer, &cx).await;
    initial_frame.expire_handle_factory = cx.expire_handle_factory.clone();
    let source = Source::Init;

    process_new_events(&mut initial_frame, source, result.new_events, cx);

    let output_events = prepare_output_events::<T>(T::Time::default(), result.emitted_events);

    WrappedInitResult {
        initial_frame,
        output_events,
    }
}

pub(super) struct WrappedUpdateResult<T: Transposer> {
    pub new_frame: TransposerFrame<T>,
    pub output_events: Vec<InternalOutputEvent<T>>,
    pub exit: bool,
}

pub(super) async fn handle_input<T: Transposer>(
    frame: Arc<RwLock<TransposerFrame<T>>>,
    time: T::Time,
    inputs: &[T::Input],
) -> WrappedUpdateResult<T> {
    // get a read lock for clone, drop right away
    // TODO don't clone for this
    let lock = frame.read().unwrap();
    let mut new_frame = lock.clone();
    std::mem::drop(lock);

    new_frame.time = time;
    let cx = TransposerContext::new(new_frame.expire_handle_factory.clone());
    let result = new_frame.transposer.handle_input(time, inputs, &cx).await;
    new_frame.expire_handle_factory = cx.expire_handle_factory.clone();
    let source = Source::Input(time);
    let result = handle_update_result(new_frame, source, result, cx);

    result
}

pub(super) async fn handle_scheduled<T: Transposer>(
    frame: Arc<RwLock<TransposerFrame<T>>>,
    event: Arc<InternalScheduledEvent<T>>,
) -> WrappedUpdateResult<T> {
    // get a read lock for clone, drop right away
    // TODO don't clone for this
    let lock = frame.read().unwrap();
    let mut new_frame = lock.clone();
    std::mem::drop(lock);

    new_frame.time = event.time;
    let cx = TransposerContext::new(new_frame.expire_handle_factory.clone());
    let result = new_frame
        .transposer
        .handle_scheduled(event.time, &event.payload, &cx)
        .await;
    new_frame.expire_handle_factory = cx.expire_handle_factory.clone();
    let source = Source::Schedule(event);
    let result = handle_update_result(new_frame, source, result, cx);

    result
}

fn handle_update_result<T: Transposer>(
    mut frame: TransposerFrame<T>,
    source: Source<T>,
    result: UpdateResult<T>,
    cx: TransposerContext,
) -> WrappedUpdateResult<T> {
    let time = source.time();
    process_new_events(&mut frame, source, result.new_events, cx);

    remove_expired_events(&mut frame, result.expired_events);

    let output_events = prepare_output_events::<T>(time, result.emitted_events);

    WrappedUpdateResult {
        new_frame: frame,
        output_events,
        exit: result.exit,
    }
}

fn process_new_events<T: Transposer>(
    frame: &mut TransposerFrame<T>,
    source: Source<T>,
    events: Vec<ScheduledEvent<T>>,
    mut cx: TransposerContext,
) {
    for (index, event) in events.into_iter().enumerate() {
        // create new event
        let new_event = InternalScheduledEvent {
            source: source.clone(),
            source_index: index,
            expire_handle: cx.new_expire_handles.remove(&index),
            time: event.timestamp,
            payload: event.payload,
        };

        // store this event in an arc forever. It will never be modified.
        let new_event = Arc::new(new_event);

        // add an expire handle if we need to.
        if let Some(handle) = new_event.expire_handle {
            frame
                .expire_handles
                .insert(handle, Arc::downgrade(&new_event.clone()));
        }

        // add new event to schedule
        frame.schedule.insert(new_event);
    }
}

fn remove_expired_events<T: Transposer>(
    frame: &mut TransposerFrame<T>,
    expired_events: Vec<ExpireHandle>,
) {
    for h in expired_events {
        if let Some(e) = frame.expire_handles.get(&h) {
            if let Some(arc) = e.upgrade() {
                frame.schedule.remove(&arc);
            }
            frame.expire_handles.remove(&h);
        }
    }
}

fn prepare_output_events<T: Transposer>(
    time: T::Time,
    raw_payloads: Vec<T::Output>,
) -> Vec<InternalOutputEvent<T>> {
    raw_payloads
        .into_iter()
        .map(|payload| Event {
            timestamp: time,
            payload,
        })
        .collect()
}
