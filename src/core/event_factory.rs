use super::event::{Event, EventContent};
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::Relaxed;

pub struct EventFactory {
    current_id: AtomicUsize,
}

impl EventFactory {
    pub fn new() -> Self {
        Self {
            current_id: AtomicUsize::new(0),
        }
    }
    pub fn new_event<T: Clone>(&self, content: EventContent<T>) -> Event<T> {
        let id = self.current_id.fetch_add(1, Relaxed);
        Event { id, content }
    }
}
