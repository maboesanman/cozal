use core::cell::RefCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use std::lazy::SyncOnceCell;

pub struct LazyState<S> {
    value:  SyncOnceCell<Box<S>>,
    status: RefCell<LazyStateStatus>,
}
pub enum LazyStateStatus {
    Requested(Waker),
    Pending,
}

impl<'a, S> Future for &'a LazyState<S> {
    type Output = &'a S;

    fn poll(self: Pin<&'_ mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this: &'_ mut &'a _ = self.get_mut();
        let lazy_state: &'a _ = *this;

        match lazy_state.value.get() {
            Some(s) => Poll::Ready(s),
            None => {
                lazy_state
                    .status
                    .replace(LazyStateStatus::Requested(cx.waker().clone()));
                Poll::Pending
            },
        }
    }
}

impl<S> LazyState<S> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self, state: S, skip_wake: bool) -> Result<(), Box<S>> {
        self.value.set(Box::new(state))?;
        if let LazyStateStatus::Requested(w) = self.status.replace(LazyStateStatus::Pending) {
            if !skip_wake {
                w.wake();
            }
        }
        Ok(())
    }

    pub fn requested(&self) -> bool {
        let a = self.status.borrow();
        matches!(*a, LazyStateStatus::Requested(_))
    }
}

impl<S> Default for LazyState<S> {
    fn default() -> Self {
        Self {
            value:  SyncOnceCell::new(),
            status: RefCell::new(LazyStateStatus::Pending),
        }
    }
}
