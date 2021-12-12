use core::task::Waker;
use std::sync::Arc;
use std::task::Wake;

pub struct DummyWaker;

impl DummyWaker {
    pub fn new() -> Waker {
        Arc::new(Self).into()
    }
}

impl Wake for DummyWaker {
    fn wake(self: Arc<Self>) {}
}
