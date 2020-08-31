use futures::task::{waker, ArcWake};
use std::{
    sync::{atomic::AtomicUsize, Arc},
    task::Waker,
};

pub struct DebugWaker<'a> {
    inner: Waker,
    count: usize,
    current_count: &'a AtomicUsize,
}

impl<'a> ArcWake for DebugWaker<'a> {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        println!(
            "wake up {:?} / {:?}",
            arc_self.count, arc_self.current_count
        );
        arc_self.inner.wake_by_ref();
    }
}

pub struct DebugWakerFactory {
    current: AtomicUsize,
}

impl DebugWakerFactory {
    pub fn new() -> Self {
        DebugWakerFactory {
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
            inner,
            count,
            current_count: &self.current,
        };
        let arc_debug = Arc::new(debug);
        waker(arc_debug)
    }
}
