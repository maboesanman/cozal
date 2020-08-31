use super::transposer::Transposer;
use super::{
    transposer_event::InternalTransposerEvent,
    transposer_expire_handle::{ExpireHandle, ExpireHandleFactory},
};
use im::{HashMap, OrdSet};
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct TransposerFrame<T: Transposer> {
    // this is an Arc because we might not change the transposer, and therefore don't need to save a copy.
    pub transposer: Arc<T>,

    // schedule and expire_handles
    pub schedule: OrdSet<InternalTransposerEvent<T>>,
    pub expire_handles: HashMap<ExpireHandle, InternalTransposerEvent<T>>,

    pub current_expire_handle: ExpireHandleFactory,
    // todo: add constants for the current randomizer
}
