use std::sync::{Arc, Weak};
use std::task::{Wake, Waker};

use parking_lot::Mutex;

use super::dummy_waker::DummyWaker;

pub struct ReplaceWaker {
    inner: Mutex<ReplaceWakerInner>,
}

struct ReplaceWakerInner {
    waker: Waker,
    woken: bool,
}

impl ReplaceWaker {
    pub fn new_empty() -> Weak<Self> {
        Weak::new()
    }

    // return whether or not this waker should be used to poll right away
    pub fn register(weak: &mut Weak<Self>, waker: Waker) -> Option<Waker> {
        let arc_self = Self::get_arc_self(weak);

        let mut lock = arc_self.inner.lock();
        lock.waker = waker;

        if !lock.woken {
            return None // no need for a waker, we weren't woken.
        }

        lock.woken = false;
        let new_waker = arc_self.clone();
        Some(new_waker.into())
    }

    pub fn get_waker(weak: &mut Weak<Self>) -> Waker {
        Self::get_arc_self(weak).into()
    }

    fn get_arc_self(weak: &mut Weak<Self>) -> Arc<Self> {
        match weak.upgrade() {
            Some(a) => a,
            None => {
                // this means the waker was dropped; need a new one.
                let arc = Arc::new(ReplaceWaker {
                    inner: Mutex::new(ReplaceWakerInner {
                        waker: DummyWaker::dummy(),
                        woken: false,
                    }),
                });
                *weak = Arc::downgrade(&arc);
                arc
            },
        }
    }
}

impl Wake for ReplaceWaker {
    fn wake(self: Arc<Self>) {
        let mut lock = self.inner.lock();
        lock.woken = true;
        lock.waker.wake_by_ref();
    }
}
