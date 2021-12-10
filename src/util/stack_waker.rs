use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Weak};
use std::task::{Wake, Waker};

use parking_lot::{RwLock, RwLockUpgradableReadGuard};

pub struct StackWaker<K: Eq + Hash> {
    inner: RwLock<StackWakerInner<K>>,
}

struct StackWakerInner<K: Eq + Hash> {
    wakers: HashMap<K, Waker>,
}

impl<K: Eq + Hash> StackWaker<K> {
    pub fn new_empty() -> Weak<Self> {
        Weak::new()
    }

    // return whether or not this waker should be used to poll right away
    pub fn register(weak: &mut Weak<Self>, key: K, waker: Waker) -> Option<Arc<Self>> {
        if let Some(this) = weak.upgrade() {
            let lock = this.inner.upgradable_read();
            if !lock.wakers.is_empty() {
                let mut lock = RwLockUpgradableReadGuard::upgrade(lock);
                let _ = lock.wakers.insert(key, waker);
            } else {
                drop(lock);
            }
            return None
        }

        let mut wakers = HashMap::with_capacity(1);
        let _ = wakers.insert(key, waker);
        let new_waker = StackWaker {
            inner: RwLock::new(StackWakerInner {
                wakers,
            }),
        };
        let new_arc = Arc::new(new_waker);
        *weak = Arc::downgrade(&new_arc);
        Some(new_arc)
    }

    pub fn is_empty(weak: &Weak<Self>) -> bool {
        match weak.upgrade() {
            Some(this) => this.inner.read().wakers.is_empty(),
            None => true,
        }
    }
}

impl<K: Eq + Hash> Wake for StackWaker<K> {
    fn wake(self: Arc<Self>) {
        for (_k, w) in self.inner.write().wakers.drain() {
            w.wake();
        }
    }
}
