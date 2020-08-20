use super::transposer::Transposer;
use crate::core::event::event::Event;
use std::sync::Arc;

pub enum TriggerEvent<T: Transposer> {
    External(Arc<Event<T::External>>),
    Internal(Arc<Event<T::Internal>>),
}
