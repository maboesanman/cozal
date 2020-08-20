use super::transposer_context::TransposerContext;
use super::trigger_event::TriggerEvent;
use crate::core::event::event::Event;
use async_trait::async_trait;

pub struct InitResult<T: Transposer> {
    pub new_updater: T,
    pub new_events: Vec<Event<T::Internal>>,
    pub emitted_events: Vec<Event<T::Out>>,
}

pub struct UpdateResult<T: Transposer> {
    pub new_updater: Option<T>,
    // all these events must be in the future
    pub expired_events: Vec<u64>,
    pub new_events: Vec<Event<T::Internal>>,
    pub emitted_events: Vec<Event<T::Out>>,
}

// it is recommended to use immutable structure sharing data types inside update.
#[async_trait]
pub trait Transposer: Clone + Unpin + Send {
    type External: Clone + Unpin + Send;
    type Internal: Clone + Unpin + Send;
    type Out: Clone + Unpin + Send;

    // initialize the state of your transposer.
    async fn init(cx: &TransposerContext) -> InitResult<Self>;

    // process events and produce a new state.
    async fn update<'a>(
        &'a self,
        cx: &TransposerContext,
        event: &'a TriggerEvent<Self>,
    ) -> UpdateResult<Self>;

    // filter out events you know you can't do anything with.
    // this reduces the amount of events you have to remember for rollback to work
    fn can_process(_event: &Event<Self::External>) -> bool {
        true
    }
}
