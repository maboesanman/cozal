use std::{pin::Pin, task::{Context, Poll}};

use futures::Future;
use take_mut::{self, take_or_recover};

pub(super) enum LazyState<S: Sized> {
    Ready(S),
    Requested,
    Pending,
}

pub struct LazyStateFuture<'a, S: Sized> {
    lazy_state: &'a mut LazyState<S>
}

impl<'a, S: Sized> Future for LazyStateFuture<'a, S> {
    type Output = S;

    // it is ok to discard the context because this is never going to be directly executed.
    // the future which runs this will manage re-calling when it knows it has the state.
    // we don't have to worry about it.
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<S> {
        let this: &mut Self = self.get_mut();

        let lazy_state: &mut LazyState<S> = this.lazy_state;
        let mut result = Poll::Pending;
        take_or_recover(lazy_state, || LazyState::Requested, |ls| {
            match ls {
                LazyState::Ready(s) => {
                    result = Poll::Ready(s);
                    LazyState::Pending
                }
                LazyState::Requested => LazyState::Requested,
                LazyState::Pending => LazyState::Requested,
            }
        });
        result
    }
}

impl<S> LazyState<S> {
    pub fn get(&mut self) -> LazyStateFuture<'_, S> {
        LazyStateFuture {
            lazy_state: self
        }
    }

    pub fn set(&mut self, state: S) -> Result<(), S> {
        if let Self::Ready(_) = self {
            Err(state)
        } else {
            *self = Self::Ready(state);
            Ok(())
        }
    }

    pub fn destroy(self) -> Option<S> {
        match self {
            Self::Ready(s) => Some(s),
            Self::Pending => None,
            Self::Requested => None,
        }
    }
}