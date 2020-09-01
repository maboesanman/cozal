use futures::task::{waker, ArcWake};
use std::{
    sync::{atomic::AtomicUsize, Arc},
    task::Waker,
};

pub struct DebugWaker<'a> {
    name: String,
    inner: Waker,
    count: usize,
    current_count: &'a AtomicUsize,
}

impl<'a> ArcWake for DebugWaker<'a> {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        if arc_self.count == arc_self.current_count.load(std::sync::atomic::Ordering::SeqCst) {
            println!("wake {}", arc_self.name);
        } else {
            println!("wake {} OUTDATED", arc_self.name);
        }
        arc_self.inner.wake_by_ref();
    }
}

pub struct DebugWakerFactory {
    name: String,
    current: AtomicUsize,
}

impl DebugWakerFactory {
    pub fn new(name: &str) -> Self {
        DebugWakerFactory {
            name: name.to_string(),
            current: AtomicUsize::from(0),
        }
    }
    pub fn wrap_waker(&self, inner: Waker) -> Waker {
        let count = self
            .current
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        println!("new {:?}", count);
        let debug = DebugWaker {
            name: self.name.clone(),
            inner,
            count,
            current_count: &self.current,
        };
        let arc_debug = Arc::new(debug);
        waker(arc_debug)
    }
}
