use core::sync::atomic::{AtomicUsize, Ordering};
use core::task::{RawWaker, Waker};
use std::sync::Arc;
use std::task::Wake;

pub struct DummyWaker {
    wake_count: AtomicUsize,
}

pub struct DummyWakerWatcher {
    waker: Arc<DummyWaker>,
}

impl DummyWakerWatcher {
    pub fn current_count(&self) -> usize {
        self.waker.wake_count.load(Ordering::SeqCst)
    }
}

impl DummyWaker {
    pub fn new() -> (Waker, DummyWakerWatcher) {
        let wake = Self {
            wake_count: AtomicUsize::new(0),
        };
        let wake = Arc::new(wake);
        let watcher = DummyWakerWatcher {
            waker: wake.clone(),
        };
        let raw_waker = RawWaker::from(wake);
        // SAFETY: because we're using RawWaker::from(Arc<..>) we are upholding the contract.
        let waker = unsafe { Waker::from_raw(raw_waker) };
        (waker, watcher)
    }
}

impl Wake for DummyWaker {
    fn wake(self: Arc<Self>) {
        self.wake_count.fetch_add(1, Ordering::SeqCst);
    }
}
