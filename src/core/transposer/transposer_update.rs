use futures::Future;
use std::{
    pin::Pin,
    task::{Context, Poll},
};

pub struct Update<'a, Time: Copy + Ord, Out> {
    pub time: Time,
    pub future: Pin<Box<dyn Future<Output = Out> + Send + 'a>>,
    pub result: Poll<Out>,
}

impl<'a, Time: Copy + Ord, Out> Update<'a, Time, Out> {
    pub fn poll(&mut self, cx: &mut Context<'_>, until: Time) -> bool {
        let time_ready = self.time <= until;
        if let Poll::Ready(_) = self.result {
            return time_ready;
        }

        self.result = Pin::new(&mut self.future).poll(cx);
        time_ready
    }
}
