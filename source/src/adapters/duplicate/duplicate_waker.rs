use core::task::Waker;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};
use std::task::Wake;

pub struct EventWakers(Arc<Mutex<BTreeMap<usize, Waker>>>);

impl EventWakers {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(BTreeMap::new())))
    }

    pub fn get_new_waker(&self, index: usize, event_waker: Waker) -> Waker {
        let mut event_wakers = self.0.lock().unwrap();
        event_wakers.insert(index, event_waker);
        drop(event_wakers);

        let waker = DuplicateEventWaker {
            event_wakers: Arc::downgrade(&self.0),
        };
        let waker = Arc::new(waker);
        Waker::from(waker)
    }
}

pub struct DuplicateEventWaker {
    event_wakers: Weak<Mutex<BTreeMap<usize, Waker>>>,
}

impl Wake for DuplicateEventWaker {
    fn wake(self: Arc<Self>) {
        if let Some(event_wakers) = self.event_wakers.upgrade() {
            let mut event_wakers_ref = event_wakers.lock().unwrap();
            for (_, waker) in event_wakers_ref.split_off(&0).into_iter() {
                waker.wake()
            }
            drop(event_wakers_ref);
        }
    }
}
