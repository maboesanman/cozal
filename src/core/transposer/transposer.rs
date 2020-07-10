use crate::core::event::event::EventContent;
use super::schedule_event::ScheduleEvent;
use futures::Future;
use std::pin::Pin;

pub struct InitResult<U: Transposer> {
    pub new_updater: U,
    pub new_events: Vec<EventContent<U::Internal>>,
    pub emitted_events: Vec<EventContent<U::Out>>,
}

pub struct UpdateResult<U: Transposer> {
    pub new_updater: U,
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

    fn init() -> Pin<Box<dyn Future<Output = InitResult<Self>>>>;
    fn update(
        &self,
        event: ScheduleEvent<Self::In, Self::Internal>,
    ) -> Pin<Box<dyn Future<Output = UpdateResult<Self>>>>;
}
