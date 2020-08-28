use super::transposer::Transposer;
use super::transposer_event::InternalTransposerEvent;
use im::{HashMap, OrdSet};
use std::sync::{Arc, Weak};

#[derive(Clone)]
pub(super) struct TransposerFrame<T: Transposer> {
    // this is an Arc because we might not change the transposer, and therefore don't need to save a copy.
    pub transposer: Arc<T>,

    // schedule and expire_handles
    pub schedule: OrdSet<Arc<InternalTransposerEvent<T>>>,
    pub expire_handles: HashMap<u64, Weak<InternalTransposerEvent<T>>>,

    pub current_expire_handle: u64,
    // todo: add constants for the current randomizer
}
