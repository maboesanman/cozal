use core::pin::Pin;
use core::task::{Context, Waker};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::RwLock;
use std::{
    sync::{Arc, Mutex, Weak},
    task::Wake,
};

use pin_project::pin_project;

use crate::source::{Source, SourcePoll};

#[pin_project]
struct Original<Src: Source>
where
    Src::Event: Clone,
{
    #[pin]
    source: Src,
    children: BTreeMap<usize, Weak<DuplicateInner<Src>>>,
    wakers: Arc<Mutex<BTreeMap<usize, Waker>>>,
}

struct DuplicateWaker {
    wakers: Weak<Mutex<BTreeMap<usize, Waker>>>,
}

impl Wake for DuplicateWaker {
    fn wake(self: Arc<Self>) {
        if let Some(wakers) = self.wakers.upgrade() {
            let mut wakers_ref = wakers.lock().unwrap();
            for (_, waker) in wakers_ref.split_off(&0).into_iter() {
                waker.wake()
            }
            core::mem::drop(wakers_ref);
        }
    }
}

enum RollbackEvent<Time: Ord + Copy, Event> {
    Event { time: Time, event: Event },
    Rollback { time: Time },
}

impl<Time: Ord + Copy, Event> Clone for RollbackEvent<Time, Event>
where
    Event: Clone,
{
    fn clone(&self) -> Self {
        match self {
            RollbackEvent::Event { time, event } => RollbackEvent::Event {
                time: *time,
                event: event.clone(),
            },
            RollbackEvent::Rollback { time } => RollbackEvent::Rollback { time: *time },
        }
    }
}

impl<Time: Ord + Copy, Event> RollbackEvent<Time, Event> {
    fn time(&self) -> &Time {
        match self {
            Self::Event { time, .. } => time,
            Self::Rollback { time, .. } => time,
        }
    }
}

impl<Time: Ord + Copy, Event> Ord for RollbackEvent<Time, Event> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time().cmp(other.time())
    }
}

impl<Time: Ord + Copy, Event> PartialOrd for RollbackEvent<Time, Event> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<Time: Ord + Copy, Event> Eq for RollbackEvent<Time, Event> {}

impl<Time: Ord + Copy, Event> PartialEq for RollbackEvent<Time, Event> {
    fn eq(&self, other: &Self) -> bool {
        match self.cmp(other) {
            Ordering::Equal => true,
            _ => false,
        }
    }
}

impl<Time: Ord + Copy, Event, State> Into<SourcePoll<Time, Event, State>>
    for RollbackEvent<Time, Event>
{
    fn into(self) -> SourcePoll<Time, Event, State> {
        match self {
            RollbackEvent::Event { time, event } => {
                return SourcePoll::Event(event, time);
            }
            RollbackEvent::Rollback { time } => {
                return SourcePoll::Rollback(time);
            }
        }
    }
}

struct DuplicateInner<Src: Source>
where
    Src::Event: Clone,
{
    index: usize,

    // treat this as pinned
    original: Arc<RwLock<Original<Src>>>,
    events: RwLock<BTreeSet<Arc<RollbackEvent<Src::Time, Src::Event>>>>,
}

pub struct Duplicate<Src: Source>
where
    Src::Event: Clone,
{
    inner: Arc<DuplicateInner<Src>>,
}

impl<Src: Source> Duplicate<Src>
where
    Src::Event: Clone,
{
    pub fn new(source: Src) -> Self {
        let original = Original {
            source: source,
            children: BTreeMap::new(),
            wakers: Arc::new(Mutex::new(BTreeMap::new())),
        };
        let original = RwLock::new(original);
        let original = Arc::new(original);

        let inner = DuplicateInner {
            index: 0,
            original,
            events: RwLock::new(BTreeSet::new()),
        };
        let inner = Arc::new(inner);
        let mut original_mut = inner.original.write().unwrap();
        let children = &mut original_mut.children;
        children.insert(0, Arc::downgrade(&inner));
        core::mem::drop(children);
        core::mem::drop(original_mut);

        Self { inner }
    }

    fn get_waker(&self) -> Waker {
        let original = self.inner.original.read().unwrap();
        let wakers = Arc::downgrade(&original.wakers);
        let waker = DuplicateWaker { wakers };
        let waker = Arc::new(waker);
        Waker::from(waker)
    }
}

impl<Src: Source> Clone for Duplicate<Src>
where
    Src::Event: Clone,
{
    fn clone(&self) -> Self {
        let original = self.inner.original.clone();
        let original_ref = original.read().unwrap();
        let children = &original_ref.children;
        let max_index = *children.last_key_value().unwrap().0;
        core::mem::drop(children);
        core::mem::drop(original_ref);
        let index = max_index + 1;
        let inner = DuplicateInner {
            index,
            original,
            events: RwLock::new(BTreeSet::new()),
        };
        let inner = Arc::new(inner);
        let mut original_mut = inner.original.write().unwrap();
        let children = &mut original_mut.children;
        children.insert(index, Arc::downgrade(&inner));
        core::mem::drop(children);
        core::mem::drop(original_mut);

        Self { inner }
    }
}

impl<Src: Source> Source for Duplicate<Src>
where
    Src::Event: Clone,
{
    type Time = Src::Time;

    type Event = Src::Event;

    type State = Src::State;

    fn poll(
        self: Pin<&mut Self>,
        poll_time: Self::Time,
        cx: &mut Context<'_>,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, Self::State> {
        let original_lock = self.inner.original.read().unwrap();
        let mut wakers_lock = original_lock.wakers.lock().unwrap();
        wakers_lock.insert(self.inner.index, cx.waker().clone());
        core::mem::drop(wakers_lock);
        core::mem::drop(original_lock);

        // if original_lock.children.get(self.inner.index)
        let events_lock = self.inner.events.read().unwrap();
        let mut first = events_lock.first();
        if let Some(x) = first {
            if x.time() > &poll_time {
                first = None;
            }
        }
        let has_event = first.is_some();
        core::mem::drop(events_lock);

        if has_event {
            let mut events_lock = self.inner.events.write().unwrap();
            // this works because the only other writers are other duplicates, which are strictly additive.
            // more events could be added, but the first one will be no later than the first one from before we relocked.
            let first = events_lock.pop_first().unwrap();
            // try to pull the event out of the arc; clone if there are other references
            let owned = Arc::try_unwrap(first).unwrap_or_else(|a| (*a).clone());
            return owned.into();
        }

        let waker = self.get_waker();
        let mut context = Context::from_waker(&waker);

        let mut original_lock = self.inner.original.write().unwrap();
        let source: Pin<&mut Src> = unsafe { Pin::new_unchecked(&mut original_lock.source) };
        let poll = source.poll(poll_time, &mut context);
        core::mem::drop(original_lock);

        match poll {
            SourcePoll::Pending => SourcePoll::Pending,
            SourcePoll::Rollback(t) => {
                let rollback_event = RollbackEvent::Rollback { time: t };
                let rollback_event = Arc::new(rollback_event);

                let original_lock = self.inner.original.read().unwrap();
                for (i, dup) in original_lock.children.iter() {
                    if *i != self.inner.index {
                        if let Some(dup) = dup.upgrade() {
                            let mut events_lock = dup.events.write().unwrap();
                            events_lock.insert(rollback_event.clone());
                            core::mem::drop(events_lock);
                        }
                    }
                }
                core::mem::drop(original_lock);

                SourcePoll::Rollback(t)
            }
            SourcePoll::Event(e, t) => {
                let rollback_event = RollbackEvent::Event {
                    event: e.clone(),
                    time: t,
                };
                let rollback_event = Arc::new(rollback_event);

                let original_lock = self.inner.original.read().unwrap();
                for (i, dup) in original_lock.children.iter() {
                    if *i != self.inner.index {
                        if let Some(dup) = dup.upgrade() {
                            let mut events_lock = dup.events.write().unwrap();
                            events_lock.insert(rollback_event.clone());
                            core::mem::drop(events_lock);
                        }
                    }
                }
                core::mem::drop(original_lock);

                SourcePoll::Event(e, t)
            }
            SourcePoll::Scheduled(s, t) => {
                let events_lock = self.inner.events.read().unwrap();

                for e in events_lock.iter() {
                    if let RollbackEvent::Event { time, .. } = **e {
                        if time < t {
                            return SourcePoll::Scheduled(s, time);
                        } else {
                            break;
                        }
                    }
                }
                core::mem::drop(events_lock);

                return SourcePoll::Scheduled(s, t);
            }
            SourcePoll::Ready(s) => {
                let events_lock = self.inner.events.read().unwrap();

                for e in events_lock.iter() {
                    if let RollbackEvent::Event { time, .. } = **e {
                        return SourcePoll::Scheduled(s, time);
                    }
                }
                core::mem::drop(events_lock);

                return SourcePoll::Ready(s);
            }
        }
    }

    fn poll_forget(
        self: Pin<&mut Self>,
        poll_time: Self::Time,
        cx: &mut Context<'_>,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, Self::State> {
        let original_lock = self.inner.original.read().unwrap();
        let mut wakers_lock = original_lock.wakers.lock().unwrap();
        wakers_lock.insert(self.inner.index, cx.waker().clone());
        core::mem::drop(wakers_lock);
        core::mem::drop(original_lock);

        // if original_lock.children.get(self.inner.index)
        let events_lock = self.inner.events.read().unwrap();
        let mut first = events_lock.first();
        if let Some(x) = first {
            if x.time() > &poll_time {
                first = None;
            }
        }
        let has_event = first.is_some();
        core::mem::drop(events_lock);

        if has_event {
            let mut events_lock = self.inner.events.write().unwrap();
            // this works because the only other writers are other duplicates, which are strictly additive.
            // more events could be added, but the first one will be no later than the first one from before we relocked.
            let first = events_lock.pop_first().unwrap();
            // try to pull the event out of the arc; clone if there are other references
            let owned = Arc::try_unwrap(first).unwrap_or_else(|a| (*a).clone());
            return owned.into();
        }

        let waker = self.get_waker();
        let mut context = Context::from_waker(&waker);

        let mut original_lock = self.inner.original.write().unwrap();
        let source: Pin<&mut Src> = unsafe { Pin::new_unchecked(&mut original_lock.source) };
        let poll = source.poll_forget(poll_time, &mut context);
        core::mem::drop(original_lock);

        match poll {
            SourcePoll::Pending => SourcePoll::Pending,
            SourcePoll::Rollback(t) => {
                let rollback_event = RollbackEvent::Rollback { time: t };
                let rollback_event = Arc::new(rollback_event);

                let original_lock = self.inner.original.read().unwrap();
                for (i, dup) in original_lock.children.iter() {
                    if *i != self.inner.index {
                        if let Some(dup) = dup.upgrade() {
                            let mut events_lock = dup.events.write().unwrap();
                            events_lock.insert(rollback_event.clone());
                            core::mem::drop(events_lock);
                        }
                    }
                }
                core::mem::drop(original_lock);

                SourcePoll::Rollback(t)
            }
            SourcePoll::Event(e, t) => {
                let rollback_event = RollbackEvent::Event {
                    event: e.clone(),
                    time: t,
                };
                let rollback_event = Arc::new(rollback_event);

                let original_lock = self.inner.original.read().unwrap();
                for (i, dup) in original_lock.children.iter() {
                    if *i != self.inner.index {
                        if let Some(dup) = dup.upgrade() {
                            let mut events_lock = dup.events.write().unwrap();
                            events_lock.insert(rollback_event.clone());
                            core::mem::drop(events_lock);
                        }
                    }
                }
                core::mem::drop(original_lock);

                SourcePoll::Event(e, t)
            }
            SourcePoll::Scheduled(s, t) => {
                let events_lock = self.inner.events.read().unwrap();

                for e in events_lock.iter() {
                    if let RollbackEvent::Event { time, .. } = **e {
                        if time < t {
                            return SourcePoll::Scheduled(s, time);
                        } else {
                            break;
                        }
                    }
                }
                core::mem::drop(events_lock);

                return SourcePoll::Scheduled(s, t);
            }
            SourcePoll::Ready(s) => {
                let events_lock = self.inner.events.read().unwrap();

                for e in events_lock.iter() {
                    if let RollbackEvent::Event { time, .. } = **e {
                        return SourcePoll::Scheduled(s, time);
                    }
                }
                core::mem::drop(events_lock);

                return SourcePoll::Ready(s);
            }
        }
    }

    fn poll_events(
        self: Pin<&mut Self>,
        poll_time: Self::Time,
        cx: &mut Context<'_>,
    ) -> crate::source::SourcePoll<Self::Time, Self::Event, ()> {
        let original_lock = self.inner.original.read().unwrap();
        let mut wakers_lock = original_lock.wakers.lock().unwrap();
        wakers_lock.insert(self.inner.index, cx.waker().clone());
        core::mem::drop(wakers_lock);
        core::mem::drop(original_lock);

        // if original_lock.children.get(self.inner.index)
        let events_lock = self.inner.events.read().unwrap();
        let mut first = events_lock.first();
        if let Some(x) = first {
            if x.time() > &poll_time {
                first = None;
            }
        }
        let has_event = first.is_some();
        core::mem::drop(events_lock);

        if has_event {
            let mut events_lock = self.inner.events.write().unwrap();
            // this works because the only other writers are other duplicates, which are strictly additive.
            // more events could be added, but the first one will be no later than the first one from before we relocked.
            let first = events_lock.pop_first().unwrap();
            // try to pull the event out of the arc; clone if there are other references
            let owned = Arc::try_unwrap(first).unwrap_or_else(|a| (*a).clone());
            return owned.into();
        }

        let waker = self.get_waker();
        let mut context = Context::from_waker(&waker);

        let mut original_lock = self.inner.original.write().unwrap();
        let source: Pin<&mut Src> = unsafe { Pin::new_unchecked(&mut original_lock.source) };
        let poll = source.poll_events(poll_time, &mut context);
        core::mem::drop(original_lock);

        match poll {
            SourcePoll::Pending => SourcePoll::Pending,
            SourcePoll::Rollback(t) => {
                let rollback_event = RollbackEvent::Rollback { time: t };
                let rollback_event = Arc::new(rollback_event);

                let original_lock = self.inner.original.read().unwrap();
                for (i, dup) in original_lock.children.iter() {
                    if *i != self.inner.index {
                        if let Some(dup) = dup.upgrade() {
                            let mut events_lock = dup.events.write().unwrap();
                            events_lock.insert(rollback_event.clone());
                            core::mem::drop(events_lock);
                        }
                    }
                }
                core::mem::drop(original_lock);

                SourcePoll::Rollback(t)
            }
            SourcePoll::Event(e, t) => {
                let rollback_event = RollbackEvent::Event {
                    event: e.clone(),
                    time: t,
                };
                let rollback_event = Arc::new(rollback_event);

                let original_lock = self.inner.original.read().unwrap();
                for (i, dup) in original_lock.children.iter() {
                    if *i != self.inner.index {
                        if let Some(dup) = dup.upgrade() {
                            let mut events_lock = dup.events.write().unwrap();
                            events_lock.insert(rollback_event.clone());
                            core::mem::drop(events_lock);
                        }
                    }
                }
                core::mem::drop(original_lock);

                SourcePoll::Event(e, t)
            }
            SourcePoll::Scheduled(s, t) => {
                let events_lock = self.inner.events.read().unwrap();

                for e in events_lock.iter() {
                    if let RollbackEvent::Event { time, .. } = **e {
                        if time < t {
                            return SourcePoll::Scheduled(s, time);
                        } else {
                            break;
                        }
                    }
                }
                core::mem::drop(events_lock);

                return SourcePoll::Scheduled(s, t);
            }
            SourcePoll::Ready(s) => {
                let events_lock = self.inner.events.read().unwrap();

                for e in events_lock.iter() {
                    if let RollbackEvent::Event { time, .. } = **e {
                        return SourcePoll::Scheduled(s, time);
                    }
                }
                core::mem::drop(events_lock);

                return SourcePoll::Ready(s);
            }
        }
    }
}
