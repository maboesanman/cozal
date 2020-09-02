use std::sync::atomic::Ordering::SeqCst;
use std::{num::NonZeroU64, sync::atomic::AtomicU64};

/// this is the handle that you use to expire scheduled events.
///
/// returning this is how
#[derive(Hash, Eq, PartialEq)]
pub struct ExpireHandle(NonZeroU64);

impl ExpireHandle {
    unsafe fn new_unchecked(value: u64) -> Self {
        ExpireHandle(NonZeroU64::new_unchecked(value))
    }

    pub(super) fn get(&self) -> u64 {
        self.0.get()
    }
}

impl Clone for ExpireHandle {
    fn clone(&self) -> Self {
        ExpireHandle(self.0)
    }
}

pub(super) struct ExpireHandleFactory(AtomicU64);

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
