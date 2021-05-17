use std::sync::atomic::{AtomicU64, Ordering::SeqCst};
use crate::core::transposer::expire_handle::ExpireHandle;

#[derive(Debug)]
pub struct ExpireHandleFactory(AtomicU64);

impl ExpireHandleFactory {
    pub fn new() -> Self {
        ExpireHandleFactory(AtomicU64::from(1))
    }

    pub fn next(&self) -> ExpireHandle {
        unsafe { ExpireHandle::new_unchecked(self.0.fetch_add(1, SeqCst)) }
    }
}

impl Clone for ExpireHandleFactory {
    fn clone(&self) -> Self {
        let internal = self.0.load(SeqCst);
        ExpireHandleFactory(AtomicU64::from(internal))
    }
}