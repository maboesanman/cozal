use super::{
    context::{InitContext, UpdateContext},
    expire_handle::ExpireHandleFactory,
    internal_scheduled_event::{InternalScheduledEvent, Source},
    transposer_frame::TransposerFrame,
    ExpireHandle, InternalOutputEvent, ScheduledEvent, Transposer,
};
use crate::core::Event;
use im::{HashMap, OrdSet};
use std::sync::Arc;

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

    let mut cx = InitContext::new(initial_frame.expire_handle_factory.clone());
    T::init_events(&mut initial_frame.transposer, &mut cx).await;
    let (new_events, emitted_events, expire_handle_factory, mut new_expire_handles) = cx.destroy();
    initial_frame.expire_handle_factory = expire_handle_factory;
    let source = Source::Init;

    process_new_events(
        &mut initial_frame,
        source,
        new_events,
        &mut new_expire_handles,
    );

    let output_events = prepare_output_events::<T>(T::Time::default(), emitted_events);

    WrappedInitResult {
        initial_frame,
        output_events,
    }
}

pub(super) struct WrappedUpdateResult<T: Transposer> {
    pub frame: TransposerFrame<T>,
    pub output_events: Vec<InternalOutputEvent<T>>,
    pub exit: bool,
}

impl<T: Transposer> WrappedUpdateResult<T> {
    pub fn new<'a>(mutated_frame: TransposerFrame<T>, used_context: UpdateContext<'a, T>) -> Self {
        let UpdateContext {
            output_events,
            exit,
            ..
        } = used_context;
        WrappedUpdateResult {
            frame: mutated_frame,
            output_events,
            exit,
        }
    }
}

fn process_new_events<T: Transposer>(
    frame: &mut TransposerFrame<T>,
    source: Source<T>,
    events: Vec<ScheduledEvent<T>>,
    new_expire_handles: &mut std::collections::HashMap<usize, ExpireHandle>,
) {
    for (index, event) in events.into_iter().enumerate() {
        // create new event
        let new_event = InternalScheduledEvent {
            source: source.clone(),
            source_index: index,
            expire_handle: new_expire_handles.remove(&index),
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
