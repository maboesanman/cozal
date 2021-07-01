use core::pin::Pin;
use core::task::Context;
use futures_core::{Future, Stream};
use pin_project::pin_project;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::source::traits::Timestamp;
use crate::source::Source;

#[pin_project]
struct RealtimeInner<Src: Source>
where
    Src::Time: Timestamp,
{
    #[pin]
    source: Src,
}

pub struct RealtimeEvents<Src: Source>
where
    Src::Time: Timestamp,
{
    inner: Arc<Mutex<RealtimeInner<Src>>>,
}

pub struct RealtimeStates<Src: Source>
where
    Src::Time: Timestamp,
{
    inner: Arc<Mutex<RealtimeInner<Src>>>,
}

// this is very generic because we want this to be runtime agnostic, but runtimes have their own mechanisms for waiting.
pub fn realtime<Src: Source, SleepFut: Future, SleepFn: Fn(Instant) -> SleepFut>(
    source: Src,
    reference: <Src::Time as Timestamp>::Reference,
    sleep_fn: SleepFn,
) -> (RealtimeEvents<Src>, RealtimeStates<Src>)
where
    Src::Time: Timestamp,
{
    unimplemented!()
}

impl<Src: Source> Stream for RealtimeEvents<Src>
where
    Src::Time: Timestamp,
{
    type Item = (Src::Time, Src::Event);

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        unimplemented!()
    }
}

impl<Src: Source> RealtimeStates<Src>
where
    Src::Time: Timestamp,
{
    async fn poll_now(&self) -> Src::State {
        self.poll(Instant::now()).await
    }

    async fn poll(&self, time: Instant) -> Src::State {
        unimplemented!()
    }
}
