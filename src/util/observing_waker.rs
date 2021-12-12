use core::sync::atomic::{AtomicBool, Ordering};
use core::task::Waker;
use std::sync::Arc;
use std::task::Wake;

use super::dummy_waker::DummyWaker;

pub struct ObservingWaker {
    woken: AtomicBool,
    waker: Waker,
}

pub struct WakerObserver {
    waker: Arc<ObservingWaker>,
}

impl WakerObserver {
    pub fn woken(&self) -> bool {
        self.waker.woken.load(Ordering::SeqCst)
    }
}

impl WakerObserver {
    pub fn new(waker: Waker) -> (Waker, Self) {
        let wake = ObservingWaker {
            woken: AtomicBool::new(false),
            waker,
        };
        let waker = Arc::new(wake);
        let observer = Self {
            waker: waker.clone(),
        };

        (waker.into(), observer)
    }

    pub fn new_dummy() -> Self {
        let wake = ObservingWaker {
            woken: AtomicBool::new(false),
            waker: DummyWaker::new(),
        };
        let waker = Arc::new(wake);
        Self {
            waker,
        }
    }
}

impl Wake for ObservingWaker {
    fn wake(self: Arc<Self>) {
        self.woken.store(true, Ordering::SeqCst);
        self.waker.wake_by_ref();
    }
}
