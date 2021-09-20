use core::num::NonZeroUsize;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

use crate::source::source_poll::{AdvanceErr, SourceAdvance, SourcePollOk};
use crate::source::SourcePoll;

pub struct SourceContext<'a, 'context> {
    pub async_context: &'a mut Context<'context>,
    pub poll_channel:  usize,
    pub source_waker:  Waker,
}

impl<'a, 'context> SourceContext<'a, 'context> {
    pub fn re_borrow<'b>(&'b mut self) -> SourceContext<'b, 'context>
    where
        'a: 'b,
    {
        let async_context = &mut self.async_context;
        let poll_channel = self.poll_channel;
        let source_waker = self.source_waker.clone();

        SourceContext {
            async_context,
            poll_channel,
            source_waker,
        }
    }

    pub fn change_channel(&mut self, new_channel: usize) {
        self.poll_channel = new_channel;
    }
}

/// An interface for querying partially complete sources of [states](`Source::State`) and [events](`Source::Events`)
///
/// The [`Source`] trait is the core abstraction for the entire cozal library. Everything is designed around the idea of making chains of [`Source`]s
///
/// When a type implements Source, it models two things:
///
/// - A timestamped set of events
///
/// - A function (in the mathematical sense) mapping [`Time`](`Source::Time`) to [`State`](`Source::State`)
pub trait Source {
    /// The type used for timestamping events and states.
    type Time: Ord + Copy;

    /// The type of events emitted by the source.
    type Event;

    /// The type of states emitted by the source.
    type State;

    /// The type of any custom errors for the source.
    type Error;

    /// Attempt to retrieve the state of the source at `time`, registering the current task for wakeup in certain situations.
    ///
    /// # Return value
    ///
    /// There are several possible return values, each indicating a distinct source state for a time `t`:
    ///
    /// - [`Ready(state)`](EventStatePoll::Ready) indicates that all known events have been emitted, and that the state is ready for the requested time. New events must wake the current task in order to be retrieved, though poll may be called at any time.
    ///
    /// - [`Scheduled(state, t_s)`](EventStatePoll::Scheduled) indicates that All events at or before time `t` have been emitted, that the next event ready to be emitted is at time `t_s`, and that the state is ready for the requested time. Emitting `Scheduled` releases the source of the responsibility of waking the current task for any new information which does not change any emitted states or create any events before time `t_s`. It is still responsible for waking the task on changes that affect events or states preceding `t_s`. The source should be polled again at or after `t_s`, though what it means to be "at time t_s" is left to the consumer of the trait.
    ///
    /// - [`Event(payload, t_e)`](EventStatePoll::Event) indicates that the requested state could not be computed because the returned event must be handled before the state can be made available. The source should be immediately polled again, as it may never wake the task.
    ///
    /// - [`Rollback(t_r)`](EventStatePoll::Rollback) indicates that previously emitted information has been discovered to be incorrect, and that the caller should re-poll information it believes it needs. Specifically, all emitted events at or after time `t_r` should be discarded, as well as all states returned from [`poll`](Source::poll). Emitting rollback makes no claims about states returned from [`poll_forget`](Source::poll_forget), which should be preferred when the caller doesn't need to be informed of states being invalidated.
    ///
    /// - [`Pending`](EventStatePoll::Pending) indicates one of the other responses is not available at this time. the current thread will be woken up when progress can be made by calling poll again with the same time. You should be polling at the same time as the call which returned pending if you are responding to a task wake, or else you might not actually be able to make progress.
    fn poll(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: SourceContext<'_, '_>,
    ) -> SourcePoll<Self::Time, Self::Event, Self::State, Self::Error>;

    /// Attempt to retrieve the state of the source at `time`, registering the current task for wakeup in certain situations. Also inform the source that the state emitted from this call is exempt from the requirement to be informed of future invalidations (that the source can "forget" about this call to poll when determining how far to roll back).
    ///
    /// If you do not need to be notified that this state has been invalidated (if for example you polled in order to render to the screen, so finding out your previous frame was wrong means nothing because you can't go back and change it) then this function should be preferred.
    fn poll_forget(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: SourceContext<'_, '_>,
    ) -> SourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
        self.poll(time, cx)
    }

    /// Attempt to determine information about the set of events before `time` without generating a state. this function behaves the same as [`poll_forget`](Source::poll_forget) but returns `()` instead of [`State`](Source::State). This function should be used in all situations when the state is not actually needed, as the implementer of the trait may be able to do less work.
    ///
    /// if you do not need to use the state, this should be preferred over poll. For example, if you are simply verifying the source does not have new events before a time t, poll_ignore_state could be faster than poll (with a custom implementation).
    fn poll_events(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: SourceContext<'_, '_>,
    ) -> SourcePoll<Self::Time, Self::Event, (), Self::Error> {
        match self.poll_forget(time, cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
            Poll::Ready(Ok(result)) => Poll::Ready(Ok(match result {
                SourcePollOk::Rollback(t) => SourcePollOk::Rollback(t),
                SourcePollOk::Event(e, t) => SourcePollOk::Event(e, t),
                SourcePollOk::Scheduled(_s, t) => SourcePollOk::Scheduled((), t),
                SourcePollOk::Ready(_s) => SourcePollOk::Ready(()),
                SourcePollOk::Finalize(t) => SourcePollOk::Finalize(t),
            })),
        }
    }

    /// Inform the source that you will never poll before `time` again.
    ///
    /// calling poll before this time should result in `SourcePollError::PollAfterAdvance`
    fn advance(self: Pin<&mut Self>, _time: Self::Time) -> SourceAdvance {
        Ok(())
    }

    /// The maximum value which can be used as the channel for a poll call.
    ///
    /// all channels between 0 and max_channel() inclusive can be used as a channel.
    fn max_channel(&self) -> NonZeroUsize {
        // SAFETY: usize::MAX is never 0.
        unsafe { NonZeroUsize::new_unchecked(usize::MAX) }
    }
}
