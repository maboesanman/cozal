use super::transposer::Transposer;
use super::transposer_event::InternalTransposerEvent;
use im::{HashMap, OrdSet};
use std::{num::NonZeroU64, sync::Arc};

#[derive(Clone)]
pub struct TransposerFrame<T: Transposer> {
    // this is an Arc because we might not change the transposer, and therefore don't need to save a copy.
    pub transposer: Arc<T>,

    // schedule and expire_handles
    pub schedule: OrdSet<InternalTransposerEvent<T>>,
    pub expire_handles: HashMap<NonZeroU64, InternalTransposerEvent<T>>,

    pub current_expire_handle: NonZeroU64,
    // todo: add constants for the current randomizer
}
