use core::future::Future;
use core::pin::Pin;
use core::ptr::NonNull;
use core::task::{Context, Poll};
use std::marker::PhantomData;
use std::sync::Arc;

use futures_channel::mpsc::Receiver;

use super::update_context::UpdateContextFamily;
use super::{Arg, SubStepTime, UpdateContext, WrappedTransposer};
use crate::schedule_storage::{RefCounted, StorageFamily};
use crate::step::step::InputState;
use crate::Transposer;

pub struct Update<'almost_static, T: Transposer, S: StorageFamily, A: Arg<T, S>, Is: InputState<T>>
where
    (T, A, Is): 'almost_static,
{
    inner:           Option<UpdateInner<'almost_static, T, S>>,
    future_contents: PhantomData<(S::Transposer<WrappedTransposer<T, S>>, S::LazyState<Is>)>,
    arg:             Arc<A>,
}

struct UpdateInner<'almost_static, T: Transposer, S: StorageFamily> {
    // references context, wrapped_transposer.transposer, and args
    future: Pin<Box<dyn 'almost_static + Future<Output = S::Transposer<WrappedTransposer<T, S>>>>>,

    output_receiver:
        futures_channel::mpsc::Receiver<(T::OutputEvent, futures_channel::oneshot::Sender<()>)>,
}

async fn create_fut<
    T: Transposer,
    S: StorageFamily,
    A: Arg<T, S>,
    Is: InputState<T>,
    C: UpdateContextFamily<T, S>,
>(
    mut wrapped_transposer: S::Transposer<WrappedTransposer<T, S>>,
    arg: Arc<A>,
    time: SubStepTime<T::Time>,
    input_state: S::LazyState<Is>,
    output_sender: futures_channel::mpsc::Sender<(
        T::OutputEvent,
        futures_channel::oneshot::Sender<()>,
    )>,
) -> S::Transposer<WrappedTransposer<T, S>> {
    // update 'current time'
    let raw_time = time.raw_time();
    let wrapped_transposer_mut = wrapped_transposer.mutate();
    wrapped_transposer_mut.metadata.last_updated = raw_time;

    let passed_arg = arg.get_passed(wrapped_transposer_mut);

    // split borrow. one goes to fut, one to context.
    let transposer = &mut wrapped_transposer_mut.transposer;
    let metadata = &mut wrapped_transposer_mut.metadata;
    let input_state_provider = input_state.get_provider();

    let context = C::UpdateContext::new(time, metadata, input_state_provider, output_sender);

    A::run(transposer, context, raw_time, passed_arg).await;
    wrapped_transposer
}

impl<'almost_static, T: Transposer, S: StorageFamily, A: Arg<T, S>, Is: InputState<T>>
    Update<'almost_static, T, S, A, Is>
{
    pub fn new<C: UpdateContextFamily<T, S> + 'almost_static>(
        wrapped_transposer: S::Transposer<WrappedTransposer<T, S>>,
        arg: A,
        time: SubStepTime<T::Time>,
        input_state: S::LazyState<Is>,
    ) -> Self {
        let (output_sender, output_receiver) = futures_channel::mpsc::channel(1);
        let arg = Arc::new(arg);
        let moved_arg = Arc::clone(&arg);

        // get future, filling box if we can.
        let future = Box::pin(create_fut::<_, _, _, _, C>(
            wrapped_transposer,
            moved_arg,
            time,
            input_state,
            output_sender,
        ));

        Self {
            inner: Some(UpdateInner {
                future,
                output_receiver,
            }),
            future_contents: PhantomData,
            arg,
        }
    }

    pub fn reclaim_args(self) -> A {
        let Self {
            inner,
            arg,
            ..
        } = self;

        drop(inner);
        match Arc::try_unwrap(arg) {
            Ok(arg) => arg,
            Err(_) => panic!(),
        }
    }

    pub fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> UpdatePoll<T, S> {
        let inner = match &mut self.inner {
            Some(inner) => inner,
            None => return UpdatePoll::Pending,
        };
        match inner.future.as_mut().poll(cx) {
            Poll::Pending => match inner.output_receiver.try_next() {
                Ok(Some((o, oneshot))) => {
                    // let the send future know it has succeeded
                    let _ = oneshot.send(());
                    UpdatePoll::Event(o)
                },
                _ => UpdatePoll::Pending,
            },
            Poll::Ready(wrapped_transposer) => UpdatePoll::Ready(wrapped_transposer),
        }
    }
}

pub enum UpdatePoll<T: Transposer, S: StorageFamily> {
    Pending,
    Event(T::OutputEvent),
    Ready(S::Transposer<WrappedTransposer<T, S>>),
}
