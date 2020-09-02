use core::pin::Pin;
use futures::stream::Fuse;
use futures::task::{Context, Poll};
use futures::{Stream, StreamExt};
use pin_project::pin_project;

use crate::{
    core::event::RollbackPayload,
    core::schedule_stream::{SchedulePoll, ScheduleStream},
    core::Event,
};

use super::{
    engine_internal::{InputStreamItem, TransposerEngineInternal},
    transposer::Transposer,
};

/// A struct which implements the [`ScheduleStream`] trait for a [`Transposer`].
///
/// This implementation does the following:
/// - rollback state and replay to resolve instability in the order of the input stream.
/// -- this is useful for online multiplayer games, where the network latency can jumble inputs.
/// - respond to rollback events from the input stream.
/// - record the input events for the purpose of storing replay data.
#[pin_project]
pub struct TransposerEngine<
    'a,
    T: Transposer + 'a,
    S: Stream<Item = InputStreamItem<'a, T>> + Unpin + Send + 'a,
> {
    #[pin]
    input_stream: Fuse<S>,

    internal: TransposerEngineInternal<'a, T>,
}

impl<'a, T: Transposer + 'a, S: Stream<Item = InputStreamItem<'a, T>> + Unpin + Send + 'a>
    TransposerEngine<'a, T, S>
{
    /// create a new TransposerEngine, consuming the input stream.
    pub async fn new(input_stream: S) -> TransposerEngine<'a, T, S> {
        TransposerEngine {
            input_stream: input_stream.fuse(),
            internal: TransposerEngineInternal::new().await,
        }
    }
}

impl<'a, T: Transposer + 'a, S: Stream<Item = InputStreamItem<'a, T>> + Unpin + Send + 'a>
    ScheduleStream for TransposerEngine<'a, T, S>
{
    type Time = T::Time;
    type Item = Event<T::Time, RollbackPayload<T::Out>>;
    fn poll_next(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: &mut Context<'_>,
    ) -> SchedulePoll<Self::Time, Self::Item> {
        let projection = self.project();
        let mut input_stream = projection.input_stream;
        let internal = projection.internal;

        while let Poll::Ready(Some(event)) = input_stream.as_mut().poll_next(cx) {
            internal.insert(event);
        }
        internal.poll(time, cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.internal.size_hint()
    }
}
