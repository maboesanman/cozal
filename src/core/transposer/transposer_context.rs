use std::{sync::{Arc, atomic::AtomicU64, Mutex}, collections::HashMap};
use core::sync::atomic::Ordering::Relaxed;

pub struct TransposerContext {
    pub(crate) current_expire_handle: AtomicU64,
    pub(crate) new_expire_handles: Arc<Mutex<HashMap<u64, usize>>>,
    // todo add seeded deterministic random function
}

impl TransposerContext {
    pub fn new(current_expire_handle: u64) -> Self {
        Self {
            current_expire_handle: AtomicU64::new(current_expire_handle),
            new_expire_handles: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    // get a value that can be used to expire an event that has been scheduled.
    // the index argument is the index in the new_events array that the handle will correspond to.
    pub fn get_expire_handle(&self, index: usize) -> u64 {
        let handle = self.current_expire_handle.fetch_add(1, Relaxed);
        self.new_expire_handles.lock().unwrap().insert(handle, index);
        handle
    }
}