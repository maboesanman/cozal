use std::{pin::Pin, task::{Context, Poll}};

use futures::Future;

pub struct LazyState<S: Sized + Unpin>(LazyStateInner<S>);
pub enum LazyStateInner<S: Sized + Unpin> {
    Ready(S),
    Requested,
    Pending,
}

impl<'a, S: Sized + Unpin> Future for &'a mut LazyState<S> {
    type Output = S;

    // it is ok to discard the context because this is never going to be directly executed.
    // the future which runs this will manage re-calling when it knows it has the state.
    // we don't have to worry about it.
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<S> {
        let mut result = Poll::Pending;
        take_mut::take_or_recover(
    &mut self.get_mut().0,
    || LazyStateInner::Pending, 
    |inner| match inner {
                LazyStateInner::Ready(state) => {
                    result = Poll::Ready(state);
                    LazyStateInner::Pending
                },
                _ => LazyStateInner::Requested,
            }
        );

        result
    }
}

impl<S: Sized + Unpin> LazyState<S> {
    pub fn new() -> Self {
        Self(LazyStateInner::Pending)
    }

    pub fn set(&mut self, state: S) -> Result<(), S> {
        if let LazyState(LazyStateInner::Ready(_)) = self {
            Err(state)
        } else {
            *self = LazyState(LazyStateInner::Ready(state));
            Ok(())
        }
    }

    pub fn requested(&self) -> bool {
        match self.0 {
            LazyStateInner::Requested => true,
            _ => false,
        }
    }

    pub fn destroy(self) -> Option<S> {
        match self.0 {
            LazyStateInner::Ready(s) => Some(s),
            LazyStateInner::Pending => None,
            LazyStateInner::Requested => None,
        }
    }
}