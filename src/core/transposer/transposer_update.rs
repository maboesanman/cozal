use super::{
    transposer::Transposer, transposer_event::TransposerEvent,
    transposer_function_wrappers::WrappedUpdateResult,
};
use futures::Future;
use std::{
    pin::Pin,
    task::{Context, Poll},
};

pub(super) struct TransposerUpdate<'a, T: Transposer> {
    pub time: T::Time,
    pub events: Vec<TransposerEvent<T>>,
    pub future: Pin<Box<dyn Future<Output = WrappedUpdateResult<T>> + Send + 'a>>,
    pub result: Poll<WrappedUpdateResult<T>>,
}

impl<'a, T: Transposer> TransposerUpdate<'a, T> {
    pub fn poll(&mut self, cx: &mut Context<'_>) {
        if let Poll::Pending = self.result {
            self.result = Pin::new(&mut self.future).poll(cx);
        }
    }
}
