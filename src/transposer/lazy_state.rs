use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use std::cell::RefCell;
use std::lazy::SyncOnceCell;

pub struct LazyState<S> {
    value:  SyncOnceCell<S>,
    status: RefCell<LazyStateStatus>,
}
pub enum LazyStateStatus {
    Requested(Waker),
    Pending,
}

impl<'a, S> Future for &'a LazyState<S> {
    type Output = &'a S;

    fn poll(self: Pin<&'_ mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: we are moving S, but we never pinned any refs to it, so it's ok.
        let this: &'_ mut &'a _ = unsafe { self.get_unchecked_mut() };
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

    pub fn set(&self, state: S) -> Result<(), S> {
        self.value.set(state)?;
        self.status.replace(LazyStateStatus::Pending);
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
