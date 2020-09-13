use super::transposer::Transposer;
use super::{
    expire_handle::{ExpireHandle, ExpireHandleFactory},
    internal_scheduled_event::InternalScheduledEvent,
};
use im::{HashMap, OrdSet};
use std::sync::{Arc, Weak};

#[derive(Clone)]
pub(super) struct TransposerFrame<T: Transposer> {
    pub time: T::Time,
    pub transposer: T,

    // schedule and expire_handles
    pub schedule: OrdSet<Arc<InternalScheduledEvent<T>>>,
    pub expire_handles: HashMap<ExpireHandle, Weak<InternalScheduledEvent<T>>>,

    pub expire_handle_factory: ExpireHandleFactory,
    // todo: add constants for the current randomizer
}
