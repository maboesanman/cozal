use std::sync::atomic::Ordering::SeqCst;
use std::{num::NonZeroU64, sync::atomic::AtomicU64};

/// this is the handle that you use to expire scheduled events.
#[derive(Hash, Eq, PartialEq, Debug, Copy)]
pub struct ExpireHandle(NonZeroU64);

impl ExpireHandle {
    pub(crate) unsafe fn new_unchecked(value: u64) -> Self {
        ExpireHandle(NonZeroU64::new_unchecked(value))
    }
}

impl Clone for ExpireHandle {
    fn clone(&self) -> Self {
        ExpireHandle(self.0)
    }
}
