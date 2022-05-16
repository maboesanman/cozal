use core::num::NonZeroUsize;
use core::pin::Pin;
use core::task::Waker;

use super::{Source, SourceContext};
use crate::source::SourcePoll;

/// An interface for querying partially complete sources of [states](`Source::State`) and [events](`Source::Events`)
///
/// The [`Source`] trait is the core abstraction for the entire cozal library. Everything is designed around the idea of making chains of [`Source`]s
///
/// When a type implements Source, it models two things:
///
/// - A timestamped set of events
///
/// - A function (in the mathematical sense) mapping [`Time`](`Source::Time`) to [`State`](`Source::State`)
pub trait ConcurrentSource: Source {
    /// poll, but concurrent
    fn poll_concurrent(
        &self,
        time: Self::Time,
        cx: SourceContext,
    ) -> SourcePoll<Self::Time, Self::Event, Self::State, Self::Error>;

    /// caller must ensure this channel is not in use by any other caller.
    unsafe fn poll_concurrent_unchecked(
        &self,
        time: Self::Time,
        cx: SourceContext,
    ) -> SourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
        self.poll_concurrent(time, cx)
    }

    /// poll_forget, but concurrent
    fn poll_forget_concurrent(
        &self,
        time: Self::Time,
        cx: SourceContext,
    ) -> SourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
        self.poll_concurrent(time, cx)
    }

    /// caller must ensure this channel is not in use by any other caller.
    unsafe fn poll_forget_concurrent_unchecked(
        &self,
        time: Self::Time,
        cx: SourceContext,
    ) -> SourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
        self.poll_forget_concurrent(time, cx)
    }

    /// poll_events, but concurrent
    fn poll_events_concurrent(
        &self,
        time: Self::Time,
        all_channel_waker: Waker,
    ) -> SourcePoll<Self::Time, Self::Event, (), Self::Error>;

    /// caller must ensure this channel is not in use by any other caller.
    unsafe fn poll_events_concurrent_unchecked(
        &self,
        time: Self::Time,
        all_channel_waker: Waker,
    ) -> SourcePoll<Self::Time, Self::Event, (), Self::Error> {
        self.poll_events_concurrent(time, all_channel_waker)
    }

    /// release_channel, but concurrent
    fn release_channel_concurrent(&self, channel: usize);

    /// caller must ensure this channel is not in use by any other caller.
    unsafe fn release_channel_concurrent_unchecked(&self, channel: usize) {
        self.release_channel_concurrent(channel)
    }

    /// advance, but concurrent
    fn advance_concurrent(&self, time: Self::Time);
}
