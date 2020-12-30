use std::sync::atomic::Ordering;
use std::{
    sync::{atomic::AtomicUsize, Arc},
    task::{RawWaker, Wake, Waker},
};
pub struct DummyWaker {
    wake_count: AtomicUsize,
}

#[allow(dead_code)]
pub struct DummyWakerWatcher {
    waker: Arc<DummyWaker>,
}

#[allow(dead_code)]
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
        let waker = unsafe { Waker::from_raw(raw_waker) };
        (waker, watcher)
    }
}

impl Wake for DummyWaker {
    fn wake(self: Arc<Self>) {
        self.wake_count.fetch_add(1, Ordering::SeqCst);
    }
}
