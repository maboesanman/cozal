use core::sync::atomic::AtomicU64;
use core::sync::atomic::Ordering::SeqCst;

use crate::transposer::ExpireHandle;

#[derive(Debug)]
pub struct ExpireHandleFactory(AtomicU64);

impl ExpireHandleFactory {
    pub fn new() -> Self {
        ExpireHandleFactory(AtomicU64::new(0))
    }

    pub fn next(&self) -> ExpireHandle {
        ExpireHandle::new(self.0.fetch_add(1, SeqCst))
    }
}

impl Clone for ExpireHandleFactory {
    fn clone(&self) -> Self {
        let internal = self.0.load(SeqCst);
        ExpireHandleFactory(AtomicU64::from(internal))
    }
}
