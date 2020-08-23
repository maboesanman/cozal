use super::transposer_context::TransposerContext;
use super::{transposer_event::TransposerEvent};
use crate::core::event::event::Event;
use async_trait::async_trait;

pub struct InitResult<T: Transposer> {
    pub new_updater: T,
    pub new_events: Vec<Event<T::Time, T::Internal>>,
    pub emitted_events: Vec<T::Out>,
}

pub struct UpdateResult<T: Transposer> {
    pub new_updater: Option<T>,
    // all these events must be in the future
    pub expired_events: Vec<u64>,
    pub new_events: Vec<Event<T::Time, T::Internal>>,
    pub emitted_events: Vec<T::Out>,
}

// it is recommended to use immutable structure sharing data types inside update.
#[async_trait]
pub trait Transposer: Clone + Unpin + Send + Sync {
    type Time: Copy + Ord + Default + Send + Sync;
    type External: Unpin + Send + Sync;
    type Internal: Unpin + Send + Sync;
    type Out: Unpin + Send + Sync;

    // initialize the state of your transposer.
    async fn init(cx: &TransposerContext) -> InitResult<Self>;

    // process events for a single time T and produce a new state.
    async fn update<'a>(
        &'a self,
        cx: &TransposerContext,
        // all these events have the same time.
        events: Vec<&TransposerEvent<Self>>,
    ) -> UpdateResult<Self>;

    // filter out events you know you can't do anything with.
    // this reduces the amount of events you have to remember for rollback to work
    fn can_process(_event: &Event<Self::Time, Self::External>) -> bool {
        true
    }
}
