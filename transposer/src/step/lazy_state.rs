use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use std::ops::DerefMut;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use futures_channel::oneshot::{channel, Receiver, Sender};
use futures_util::future::Shared;
use futures_util::FutureExt;
use parking_lot::Mutex;
use util::replace_mut::replace_and_return;

pub struct LazyState<S> {
    requested: Arc<AtomicU8>,
    sender:    Option<Sender<Arc<S>>>,
    receiver:  Shared<Receiver<Arc<S>>>,
}

impl<S> LazyState<S> {
    pub fn new() -> Self {
        let requested = Arc::new(AtomicU8::new(0));

        let (sender, receiver) = channel();
        let sender = Some(sender);
        let receiver = receiver.shared();

        Self {
            requested,
            sender,
            receiver,
        }
    }

    pub fn set(&mut self, state: S) -> Result<(), Arc<S>> {
        let state = Arc::new(state);
        match self.sender.take() {
            Some(sender) => sender.send(state),
            None => Err(state),
        }
    }

    pub fn requested(&self) -> bool {
        let b = self.requested.load(Ordering::SeqCst) == 1;
        print!("{b:?}");
        b
    }

    pub fn get_proxy(&self) -> LazyStateProxy<S> {
        let inner = LazyStateProxyInner::Pending(self.requested.clone(), self.receiver.clone());
        let inner = Mutex::new(inner);
        LazyStateProxy {
            inner,
        }
    }
}

impl<'a, S> Future for &'a mut LazyState<S> {
    type Output = &'a S;

    fn poll(mut self: Pin<&'_ mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.requested.fetch_max(1, Ordering::SeqCst);
        let arc = match Pin::new(&mut self.as_mut().get_mut().receiver).poll(cx) {
            Poll::Ready(arc) => arc,
            Poll::Pending => return Poll::Pending,
        };
        self.requested.store(2, Ordering::SeqCst);
        let ptr: NonNull<_> = arc.unwrap().as_ref().into();
        // SAFETY: because the arc is never modified after being set,
        // and the lifetime of the reference given out is 'a, which is the lifetime of
        // the reference to the LazyState which owns a clone of the arc, the state given here
        // will be valid until 'a.
        let state: &'a S = unsafe { ptr.as_ref() };
        Poll::Ready(state)
    }
}

pub struct LazyStateProxy<S> {
    inner: Mutex<LazyStateProxyInner<S>>,
}

#[derive(Clone)]
enum LazyStateProxyInner<S> {
    Pending(Arc<AtomicU8>, Shared<Receiver<Arc<S>>>),
    Ready(Arc<S>),
    Limbo,
}

impl<'a, S> Future for &'a LazyStateProxy<S> {
    type Output = &'a S;

    fn poll(self: Pin<&'_ mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut lock = self.get_mut().inner.lock();
        let ptr: Option<NonNull<S>> = replace_and_return(
            lock.deref_mut(), 
        || LazyStateProxyInner::Limbo,
        |inner| match inner {
            LazyStateProxyInner::Pending(requested, mut fut) => {
                requested.fetch_max(1, Ordering::SeqCst);
                match Pin::new(&mut fut).poll(cx) {
                    Poll::Ready(arc) => {
                        let arc = arc.unwrap();
                        let ptr = NonNull::from(&*arc);
                        requested.store(2, Ordering::SeqCst);
                        (LazyStateProxyInner::Ready(arc), Some(ptr))
                    },
                    Poll::Pending => {
                        (LazyStateProxyInner::Pending(requested, fut), None)
                    },
                }
            },
            LazyStateProxyInner::Ready(arc) => {
                let ptr = NonNull::from(&*arc);
                (LazyStateProxyInner::Ready(arc), Some(ptr))
            },
            LazyStateProxyInner::Limbo => panic!(),
        });
        let ptr = match ptr {
            Some(p) => p,
            None => return Poll::Pending,
        };
        // SAFETY: because the arc is never modified after being set,
        // and the lifetime of the reference given out is 'a, which is the lifetime of
        // the reference to the LazyState which owns a clone of the arc, the state given here
        // will be valid until 'a.
        let state: &'a S = unsafe { ptr.as_ref() };
        Poll::Ready(state)
    }
}
