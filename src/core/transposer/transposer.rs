use crate::core::event::event::{Event, EventContent};
use super::schedule_event::ScheduleEvent;
use futures::Future;
use std::pin::Pin;

pub struct InitResult<U: Transposer> {
    pub new_updater: U,
    pub new_events: Vec<EventContent<U::Internal>>,
    pub emitted_events: Vec<EventContent<U::Out>>,
}

pub struct UpdateResult<U: Transposer> {
    pub new_updater: Option<U>,
    pub trigger: ScheduleEvent<U::In, U::Internal>,
    // all these events must be in the future
    pub expired_events: Vec<usize>,
    pub new_events: Vec<EventContent<U::Internal>>,
    pub emitted_events: Vec<EventContent<U::Out>>,
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
        event: ScheduleEvent<Self::In, Self::Internal>,
    ) -> Pin<Box<dyn Future<Output = UpdateResult<Self>>>>;

    // filter out events you know you can't do anything with.
    // this reduces the amount of events you have to remember for rollback to work
    fn can_process(_event: &Event<Self::In>) -> bool {
        true
    }

    // calculate the most likely outcome assuming there might be missing events.
    // this is for tailoring netcode to more specific use cases to reduce "teleporting".
    fn extrapolate(
        &self,
        event: ScheduleEvent<Self::In, Self::Internal>,
    ) -> Pin<Box<dyn Future<Output = UpdateResult<Self>>>> {
        self.update(event)
    }
}
