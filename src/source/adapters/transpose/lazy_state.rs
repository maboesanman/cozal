use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use std::task::Waker;

pub struct LazyState<S>(LazyStateInner<S>);
pub enum LazyStateInner<S> {
    Ready(S),
    Requested(Waker),
    Pending,
}

impl<'a, S> Future for &'a mut LazyState<S> {
    type Output = S;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<S> {
        let mut result = Poll::Pending;
        take_mut::take_or_recover(
            &mut self.get_mut().0,
            || LazyStateInner::Pending,
            |inner| match inner {
                LazyStateInner::Ready(state) => {
                    result = Poll::Ready(state);
                    LazyStateInner::Pending
                },
                _ => LazyStateInner::Requested(cx.waker().clone()),
            },
        );

        result
    }
}

impl<S> LazyState<S> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, state: S) -> Result<(), S> {
        let mut return_value = Ok(());
        take_mut::take(&mut self.0, |inner| match inner {
            LazyStateInner::Ready(s) => {
                return_value = Err(state);
                LazyStateInner::Ready(s)
            },
            LazyStateInner::Requested(waker) => {
                waker.wake();
                LazyStateInner::Ready(state)
            },
            LazyStateInner::Pending => LazyStateInner::Ready(state),
        });

        return_value
    }

    pub fn requested(&self) -> bool {
        matches!(self.0, LazyStateInner::Requested(_))
    }

    pub fn destroy(self) -> Option<S> {
        match self.0 {
            LazyStateInner::Ready(s) => Some(s),
            LazyStateInner::Pending => None,
            LazyStateInner::Requested(_) => None,
        }
    }
}

impl<S> Default for LazyState<S> {
    fn default() -> Self {
        Self(LazyStateInner::Pending)
    }
}
