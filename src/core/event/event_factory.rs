use super::event::{Event, EventContent};
use core::sync::atomic::AtomicU64;
use core::sync::atomic::Ordering::Relaxed;

pub struct EventFactory {
    current_id: AtomicU64,
}

impl EventFactory {
    pub fn new() -> Self {
        Self {
            current_id: AtomicU64::new(0),
        }
    }
    pub fn new_event<T: Clone>(&self, content: EventContent<T>) -> Event<T> {
        let id = self.current_id.fetch_add(1, Relaxed);
        Event { id, content }
    }
}
