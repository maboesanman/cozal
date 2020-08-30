use core::pin::Pin;
use futures::stream::Fuse;
use futures::task::{Context, Poll};
use futures::{Stream, StreamExt};
use pin_project::pin_project;

use crate::{
    core::event::event::{Event, RollbackPayload},
    core::schedule_stream::schedule_stream::{SchedulePoll, ScheduleStream},
};

use super::{
    transposer::Transposer,
    transposer_engine_internal::{InputStreamItem, TransposerEngineInternal},
};
use std::fmt::Debug;

#[pin_project(project = TransposerEngineProjection)]
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
    ScheduleStream for TransposerEngine<'a, T, S>
    where T::Time: Debug
{
    type Time = T::Time;
    type Item = Event<T::Time, RollbackPayload<T::Out>>;
    fn poll_next(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: &mut Context<'_>,
    ) -> SchedulePoll<Self::Time, Self::Item> {
        let TransposerEngineProjection {
            input_stream,
            internal,
        } = self.project();
        if let Poll::Ready(Some(event)) = input_stream.poll_next(cx) {
            internal.insert(event);
        }
        internal.poll(time, cx)
    }
}

impl<'a, T: Transposer + 'a, S: Stream<Item = InputStreamItem<'a, T>> + Unpin + Send + 'a>
    TransposerEngine<'a, T, S>
    where T::Time : Debug
{
    pub async fn new(input_stream: S) -> TransposerEngine<'a, T, S> {
        TransposerEngine {
            input_stream: input_stream.fuse(),
            internal: TransposerEngineInternal::new().await,
        }
    }
}
