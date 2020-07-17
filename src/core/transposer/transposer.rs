use crate::core::event::{event_factory::EventFactory, event::{Event, EventContent, EventTimestamp}};
use super::schedule_event::ScheduleEvent;
use futures::Future;
use std::pin::Pin;

pub struct InitResult<T: Transposer> {
    pub new_updater: T,
    pub new_events: Vec<EventContent<T::Internal>>,
    pub emitted_events: Vec<EventContent<T::Out>>,
}

pub struct UpdateResult<T: Transposer> {
    pub new_updater: Option<T>,
    pub trigger: ScheduleEvent<T::In, T::Internal>,
    // all these events must be in the future
    pub expired_events: Vec<u64>,
    pub new_events: Vec<NewUpdateEvent<T::Internal>>,
    pub emitted_events: Vec<EventContent<T::Out>>,
}

pub enum NewUpdateEvent<T: Clone> {
    Event(Event<T>),
    Content(EventContent<T>),
}

impl<T: Clone> NewUpdateEvent<T> {
    pub fn content(&self) -> &EventContent<T> {
        match self {
            NewUpdateEvent::Event(e) => &e.content,
            NewUpdateEvent::Content(c) => &c,
        }
    }

    pub fn timestamp(&self) -> &EventTimestamp {
        &self.content().timestamp
    }
}

pub struct TransposerContext {
    pub event_factory: &'static EventFactory,
    // todo add seeded deterministic random function
}

pub trait Transposer: Clone + Unpin + Send {
    type In: Clone + Unpin + Send;
    type Internal: Clone + Unpin + Send;
    type Out: Clone + Unpin + Send;

    // initialize the state of your transposer.
    fn init() -> Pin<Box<dyn Future<Output = InitResult<Self>>>>;

    // process events and produce a new state.
    // it is reccomended to use immutable structure sharing data types inside update.
    fn update(
        &self,
        cx: &TransposerContext,
        event: ScheduleEvent<Self::In, Self::Internal>,
    ) -> Pin<Box<dyn Future<Output = UpdateResult<Self>>>>;

    // filter out events you know you can't do anything with.
    // this reduces the amount of events you have to remember for rollback to work
    fn can_process(_event: &Event<Self::In>) -> bool {
        true
    }
}
