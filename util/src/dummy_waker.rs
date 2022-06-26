use core::task::Waker;
use std::sync::Arc;
use std::task::Wake;

pub struct DummyWaker;

impl DummyWaker {
    pub fn dummy() -> Waker {
        Arc::new(Self).into()
    }
}

impl Wake for DummyWaker {
    fn wake(self: Arc<Self>) {}
}
