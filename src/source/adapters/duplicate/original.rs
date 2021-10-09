use core::task::Waker;
use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::{Arc, Mutex, RwLock, Weak};
use std::task::Poll;

use super::advanced::Advanced;
use super::duplicate_inner::DuplicateInner;
use super::duplicate_waker::EventWakers;
use super::PollFn;
use crate::source::adapters::duplicate::rollback_event::RollbackEvent;
use crate::source::source_poll::SourcePollOk;
use crate::source::traits::SourceContext;
use crate::source::{Source, SourcePoll};

pub struct Original<Src: Source>
where
    Src::Event: Clone,
{
    pub source: Mutex<Src>,
    children:   RwLock<BTreeMap<usize, Weak<DuplicateInner<Src>>>>,
    wakers:     EventWakers,
    advanced:   Mutex<Advanced<Src::Time>>,
}

impl<Src: Source> Original<Src>
where
    Src::Event: Clone,
{
    pub fn new(source: Src) -> Arc<Self> {
        let original = Original {
            source:   Mutex::new(source),
            children: RwLock::new(BTreeMap::new()),
            wakers:   EventWakers::new(),
            advanced: Mutex::new(Advanced::new()),
        };

        Arc::new(original)
    }

    pub fn get_next_index(&self) -> usize {
        let children = self.children.read().unwrap();

        match children.last_key_value() {
            Some((i, _)) => i + 1,
            None => 0,
        }
    }

    pub fn register_child(&self, index: usize, child: &Arc<DuplicateInner<Src>>) {
        let weak_child = Arc::downgrade(child);
        self.children.write().unwrap().insert(index, weak_child);
        self.advanced.lock().unwrap().register_new_duplicate(index);
    }

    pub fn max_channel(&self) -> NonZeroUsize {
        self.source.lock().unwrap().max_channel()
    }

    pub fn get_new_waker(&self, index: usize, event_waker: Waker) -> Waker {
        self.wakers.get_new_waker(index, event_waker)
    }

    pub fn advance(&self, time: Src::Time, index: usize) {
        if let Some(t) = self.advanced.lock().unwrap().advance(time, index) {
            let mut source_lock = self.source.lock().unwrap();

            // SAFETY: our source is structurally pinned inside a mutex inside original, which is an Arc, therefore unmoving.
            let source: Pin<&mut Src> = unsafe { Pin::new_unchecked(&mut source_lock) };
            source.advance(t);
        }
    }

    pub fn poll<State>(
        self: &Arc<Self>,
        poll_time: Src::Time,
        cx: SourceContext,
        poll_fn: PollFn<Src, State>,
        from_index: usize,
    ) -> SourcePoll<Src::Time, Src::Event, State, Src::Error> {
        let mut source_lock = self.source.lock().unwrap();

        // SAFETY: our source is structurally pinned inside a mutex inside original, which is an Arc, therefore unmoving.
        let source: Pin<&mut Src> = unsafe { Pin::new_unchecked(&mut source_lock) };
        let poll = poll_fn(source, poll_time, cx);

        // Immediately delete all stored events at or after t.
        if let Poll::Ready(Ok(SourcePollOk::Rollback(t))) = &poll {
            let children_lock = self.children.read().unwrap();
            for (_, dup) in children_lock.iter() {
                if let Some(dup) = dup.upgrade() {
                    dup.handle_rollback(*t)
                }
            }
        }

        // Now we can release the lock. Don't move this up.
        drop(source_lock);

        match poll {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Ready(Ok(poll_ok)) => Poll::Ready(Ok(match poll_ok {
                SourcePollOk::Rollback(t) => {
                    let rollback_event = RollbackEvent::Rollback {
                        time: t
                    };
                    self.distribute_event(rollback_event, from_index);

                    SourcePollOk::Rollback(t)
                },
                SourcePollOk::Event(e, t) => {
                    let rollback_event = RollbackEvent::Event {
                        event: e.clone(),
                        time:  t,
                    };
                    self.distribute_event(rollback_event, from_index);

                    SourcePollOk::Event(e, t)
                },
                SourcePollOk::Finalize(t) => {
                    let rollback_event = RollbackEvent::Finalize {
                        time: t
                    };
                    self.distribute_event(rollback_event, from_index);

                    SourcePollOk::Finalize(t)
                },
                SourcePollOk::Scheduled(s, t) => SourcePollOk::Scheduled(s, t),
                SourcePollOk::Ready(s) => SourcePollOk::Ready(s),
            })),
        }
    }

    fn distribute_event(
        &self,
        rollback_event: RollbackEvent<Src::Time, Src::Event>,
        from_index: usize,
    ) {
        let rollback_event = Arc::new(rollback_event);

        let children_lock = self.children.read().unwrap();

        // send it to all but the from_index
        for (i, dup) in children_lock.iter() {
            if *i != from_index {
                if let Some(dup) = dup.upgrade() {
                    dup.insert_rollback_event(rollback_event.clone());
                }
            }
        }
    }
}
