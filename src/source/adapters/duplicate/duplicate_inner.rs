use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::{Arc, Mutex, RwLock};
use std::task::Poll;

use super::original::Original;
use super::rollback_event::RollbackEvent;
use super::{channel_map, PollFn};
use crate::source::source_poll::SourcePollOk;
use crate::source::traits::SourceContext;
use crate::source::{Source, SourcePoll};

pub struct DuplicateInner<Src: Source>
where
    Src::Event: Clone,
{
    index: usize,

    original: Arc<Original<Src>>,
    events:   RwLock<Events<Src>>,
}

impl<Src: Source> DuplicateInner<Src>
where
    Src::Event: Clone,
{
    pub fn new(source: Src) -> Arc<Self> {
        Self::from_original(Original::new(source))
    }

    pub fn clone_inner(&self) -> Arc<Self> {
        Self::from_original(self.original.clone())
    }

    pub fn from_original(original: Arc<Original<Src>>) -> Arc<Self> {
        let index = original.get_next_index();

        let child = DuplicateInner {
            index,
            original,
            events: RwLock::new(Events::new()),
        };

        let child = Arc::new(child);

        child.original.register_child(index, &child);

        child
    }

    pub fn poll<State>(
        self: &Arc<Self>,
        poll_time: Src::Time,
        cx: SourceContext,
        poll_fn: PollFn<Src, State>,
    ) -> SourcePoll<Src::Time, Src::Event, State, Src::Error> {
        // we need to register our waker right away, even if we don't end up using the context
        let updated_cx = self.get_new_context(cx);

        // emit previously emitted stuff if we have any.
        if let Some(event) = self.poll_previously_emitted(poll_time) {
            return Poll::Ready(Ok(event.into()))
        }

        // poll the underlying source for new info
        let poll = self.original
            .poll(poll_time, updated_cx, poll_fn, self.index);

        // adjust the scheduled/ready response to account for previously emitted events.
        match poll {
            Poll::Ready(Ok(SourcePollOk::Ready(s))) => {
                todo!()
            },
            Poll::Ready(Ok(SourcePollOk::Scheduled(s, t))) => {
                todo!()
            },
            other => other
        }
    }

    pub fn max_channel(&self) -> NonZeroUsize {
        channel_map::max_channel(self.original.max_channel(), self.index)
    }

    pub fn handle_rollback(&self, time: Src::Time) {
        let mut events_lock = self.events.write().unwrap();

        // throw away all stored events at or after time t
        let key = RollbackEvent::Search {
            time,
        };
        drop(events_lock.0.split_off(&key));
    }

    pub fn insert_rollback_event(
        &self,
        rollback_event: Arc<RollbackEvent<Src::Time, Src::Event>>,
    ) -> bool {
        let mut events_lock = self.events.write().unwrap();
        events_lock.insert(rollback_event)
    }

    fn get_new_context(&self, cx: SourceContext) -> SourceContext {
        let SourceContext {
            channel,
            channel_waker,
            event_waker,
        } = cx;

        // register our waker and get the waker we can use to poll the original source.
        let event_waker = self.original.get_new_waker(self.index, event_waker);

        // map the channel via the channel_map math.
        let channel = channel_map::map(self.index, channel);

        SourceContext {
            channel,
            channel_waker,
            event_waker,
        }
    }

    fn poll_previously_emitted(
        &self,
        poll_time: Src::Time,
    ) -> Option<RollbackEvent<Src::Time, Src::Event>> {
        let events_lock = self.events.read().unwrap();
        let mut first = events_lock.0.first();

        // ignore events which occur after poll_time. don't ignore rollbacks.
        if let Some(x) = first {
            if let RollbackEvent::Event {
                time, ..
            } = x.as_ref()
            {
                if time > &poll_time {
                    first = None;
                }
            }
        }

        // record this and drop the read lock.
        let should_emit_stored = first.is_some();
        drop(events_lock);

        // be careful with rollbacks. they can't be sorted, so when one is emitted, all the duplicates need to purge cancelled events
        // rollbacks always sort first. Rolled back events are removed immediately when we first receive the rollback from the original
        if should_emit_stored {
            let mut events_lock = self.events.write().unwrap();

            // this works because the only other writers are other duplicates, which can remove events, but not rollbacks.
            // additionally, if an event is removed, a rollback must also be inserted at or before it.
            // we definitely know that whatever is first in this to_emit queue from the last lock is still the thing we want to emit.
            let first = events_lock.0.pop_first().unwrap();

            // try to pull the event out of the arc; clone if there are other references
            let owned = Arc::try_unwrap(first).unwrap_or_else(|a| (*a).clone());

            Some(owned)
        } else {
            None
        }
    }
}

struct Events<Src: Source>(BTreeSet<Arc<RollbackEvent<Src::Time, Src::Event>>>);

impl<Src: Source> Events<Src> {
    pub fn new() -> Self {
        Self(BTreeSet::new())
    }

    pub fn insert(&mut self, rollback_event: Arc<RollbackEvent<Src::Time, Src::Event>>) -> bool {
        self.0.insert(rollback_event)
    }
}
