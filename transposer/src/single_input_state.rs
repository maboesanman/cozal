use std::ptr::NonNull;

use futures_channel::oneshot::{channel, Receiver, Sender};
use parking_lot::RwLock;

use crate::step::InputState;
use crate::{StateRetriever, Transposer, TransposerInput};

/// This is the manager that transposers should use when they have a single input.
pub type SingleInputStateManager<I> = dyn StateRetriever<I>;

/// This is ONE VALID IMPLEMENTATION that backs the SingleInputStateManager.
/// Transposer can depend on SingleInputStateManager, but the specific choice of runtime
/// for your transposer only needs to be able to proivide a reference to the InputStateManager.
pub struct SingleInputState<I: TransposerInput> {
    inner: RwLock<SingleInputStateInner<I>>,
}
enum SingleInputStateInner<I: TransposerInput> {
    Empty,
    Requested(Vec<Sender<NonNull<I::InputState>>>),
    Full(Box<I::InputState>),
}

unsafe impl<I: TransposerInput> StateRetriever<I> for SingleInputState<I> {
    fn get_input_state(&self) -> Receiver<NonNull<I::InputState>> {
        let (send, recv) = channel();
        let read = self.inner.read();

        match &*read {
            SingleInputStateInner::Empty => {
                drop(read);
                let mut write = self.inner.write();
                *write = SingleInputStateInner::Requested(vec![send]);
            },
            SingleInputStateInner::Requested(_) => {
                drop(read);
                let mut write = self.inner.write();
                if let SingleInputStateInner::Requested(vec) = &mut *write {
                    vec.push(send)
                } else {
                    unreachable!()
                }
            },
            SingleInputStateInner::Full(input_state) => {
                let _ = send.send((&**input_state).into());
            },
        }

        recv
    }
}

impl<
        T: Transposer<InputStateManager = SingleInputStateManager<I>>,
        I: TransposerInput<Base = T>,
    > InputState<I::Base> for SingleInputState<I>
{
    fn new() -> Self {
        Self {
            inner: RwLock::new(SingleInputStateInner::Empty),
        }
    }

    fn get_provider(&self) -> &<I::Base as Transposer>::InputStateManager {
        self
    }
}

impl<I: TransposerInput> SingleInputState<I> {
    pub fn set_state(&self, state: I::InputState) -> Result<(), I::InputState> {
        let mut inner = self.inner.write();
        let senders = match core::mem::replace(&mut *inner, SingleInputStateInner::Empty) {
            SingleInputStateInner::Empty => Vec::new(),
            SingleInputStateInner::Requested(vec) => vec,
            SingleInputStateInner::Full(s) => {
                *inner = SingleInputStateInner::Full(s);
                return Err(state)
            },
        };
        let state = Box::new(state);

        for send in senders.into_iter() {
            let ptr: NonNull<_> = (&*state).into();
            let _ = send.send(ptr);
        }

        *inner = SingleInputStateInner::Full(state);

        Ok(())
    }
}
