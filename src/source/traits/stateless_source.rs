use core::num::NonZeroUsize;
use core::pin::Pin;

use super::source::SourceContext;
use super::Source;
use crate::source::SourcePoll;

/// An interface for calling poll_events on trait objects when state is not known.
///
/// Simply passes through poll_events to the underlying `Source`
pub trait StatelessSource {
    /// The type used for timestamping events and states.
    type Time: Ord + Copy;

    /// The type of events emitted by the stream.
    type Event: Sized;

    /// The type of any custom errors for the source.
    type Error;

    /// Attempt to determine information about the set of events before `time` without generating a state. this function behaves the same as [`poll_forget`](Source::poll_forget) but returns `()` instead of [`State`](Source::State). This function should be used in all situations when the state is not actually needed, as the implementer of the trait may be able to do less work.
    ///
    /// if you do not need to use the state, this should be preferred over poll. For example, if you are simply verifying the stream does not have new events before a time t, poll_ignore_state could be faster than poll (with a custom implementation).
    fn poll_events(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: SourceContext<'_, '_>,
    ) -> SourcePoll<Self::Time, Self::Event, (), Self::Error>;

    fn max_channels(&self) -> NonZeroUsize;
}

impl<S> StatelessSource for S
where
    S: Source,
{
    type Time = S::Time;

    type Event = S::Event;

    type Error = S::Error;

    fn poll_events(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: SourceContext<'_, '_>,
    ) -> SourcePoll<Self::Time, Self::Event, (), Self::Error> {
        S::poll_events(self, time, cx)
    }

    fn max_channels(&self) -> NonZeroUsize {
        S::max_channels(self)
    }
}
