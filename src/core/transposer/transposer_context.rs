use core::sync::atomic::Ordering::Relaxed;
use std::{
    collections::HashMap,
    sync::{atomic::AtomicU64, Arc, Mutex},
};
// todo document.
pub struct TransposerContext {
    pub(super) current_expire_handle: AtomicU64,
    pub(super) new_expire_handles: Arc<Mutex<HashMap<usize, u64>>>,
    // todo add seeded deterministic random function
}

#[allow(dead_code)]
impl TransposerContext {
    pub(super) fn new(current_expire_handle: u64) -> Self {
        Self {
            current_expire_handle: AtomicU64::new(current_expire_handle),
            new_expire_handles: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    // get a handle that can be used to expire an event that has been scheduled.
    // the index argument is the index in the new_events array that the handle will correspond to.
    pub fn get_expire_handle(&self, index: usize) -> u64 {
        let mut handles = self.new_expire_handles
            .lock()
            .unwrap();

        if handles.get(&index).is_none() {
            let handle = self.current_expire_handle.fetch_add(1, Relaxed);
            handles.insert(index, handle);
        }
        
        handles[&index]
    }

    // todo add functions to get state from other streams somehow...
}
