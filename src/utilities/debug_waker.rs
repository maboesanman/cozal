use futures::task::{waker, ArcWake};
use std::{sync::Arc, task::Waker};

pub struct DebugWaker {
    inner: Waker,
    count: usize,
}

impl ArcWake for DebugWaker {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        println!("wake up {:?}", arc_self.count);
        arc_self.inner.wake_by_ref();
    }
}

pub fn wrap_waker(inner: Waker, count: usize) -> Waker {
    println!("new {:?}", count);
    let debug = DebugWaker { inner, count };
    let arc_debug = Arc::new(debug);
    waker(arc_debug)
}
