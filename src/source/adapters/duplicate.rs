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
    source: Mutex<Src>,
    scheduled: RwLock<Option<Src::Time>>,
    children: RwLock<BTreeMap<usize, Weak<DuplicateInner<Src>>>>,
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

    // never stored; used for searching.
    Search { time: Time },
}

impl<Time: Ord + Copy, Event> Clone for RollbackEvent<Time, Event>
where
    Event: Clone,
{
    fn clone(&self) -> Self {
        match self {
            Self::Event { time, event } => Self::Event {
                time: *time,
                event: event.clone(),
            },
            Self::Rollback { time } => Self::Rollback { time: *time },
            Self::Search { time } => Self::Search { time: *time },
        }
    }
}

impl<Time: Ord + Copy, Event> RollbackEvent<Time, Event> {
    fn time(&self) -> &Time {
        match self {
            Self::Event { time, .. } => time,
            Self::Rollback { time, .. } => time,
            Self::Search { time, .. } => time,
        }
    }
}

impl<Time: Ord + Copy, Event> Ord for RollbackEvent<Time, Event> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Event { .. }, Self::Rollback { .. }) => Ordering::Greater,
            (Self::Rollback { .. }, Self::Event { .. }) => Ordering::Less,
            (Self::Rollback { .. }, Self::Search { .. }) => Ordering::Less,
            (Self::Search { .. }, Self::Rollback { .. }) => Ordering::Greater,
            (s, o) => s.time().cmp(o.time()),
        }
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
        matches!(self.cmp(other), Ordering::Equal)
    }
}

impl<Time: Ord + Copy, Event, State> Into<SourcePoll<Time, Event, State>>
    for RollbackEvent<Time, Event>
{
    fn into(self) -> SourcePoll<Time, Event, State> {
        match self {
            Self::Event { time, event } => SourcePoll::Event(event, time),
            Self::Rollback { time } => SourcePoll::Rollback(time),
            Self::Search { .. } => unreachable!(),
        }
    }
}

struct Events<Src: Source> {
    to_emit: BTreeSet<Arc<RollbackEvent<Src::Time, Src::Event>>>
}

impl<Src: Source> Events<Src> {
    pub fn new() -> Self {
        Self {
            to_emit: BTreeSet::new(),
        }
    }
}

struct DuplicateInner<Src: Source>
where
    Src::Event: Clone,
{
    index: usize,

    // treat this as pinned
    original: Arc<Original<Src>>,
    events: RwLock<Events<Src>>,
}

type PollFn<Src, State> = fn(
    Pin<&mut Src>,
    <Src as Source>::Time,
    &mut Context,
) -> SourcePoll<<Src as Source>::Time, <Src as Source>::Event, State>;

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
            source: Mutex::new(source),
            scheduled: RwLock::new(None),
            children: RwLock::new(BTreeMap::new()),
            wakers: Arc::new(Mutex::new(BTreeMap::new())),
        };
        let original = Arc::new(original);

        let inner = DuplicateInner {
            index: 0,
            original,
            events: RwLock::new(Events::new()),
        };
        let inner = Arc::new(inner);
        let mut children_mut = inner.original.children.write().unwrap();
        children_mut.insert(0, Arc::downgrade(&inner));
        core::mem::drop(children_mut);

        Self { inner }
    }

    fn get_waker(&self) -> Waker {
        let wakers = Arc::downgrade(&self.inner.original.wakers);
        let waker = DuplicateWaker { wakers };
        let waker = Arc::new(waker);
        Waker::from(waker)
    }

    fn poll_internal<State>(
        self: Pin<&mut Self>,
        poll_time: Src::Time,
        cx: &mut Context,
        poll_fn: PollFn<Src, State>,
    ) -> SourcePoll<Src::Time, Src::Event, State> {
        // store waker right away
        let mut wakers_lock = self.inner.original.wakers.lock().unwrap();
        wakers_lock.insert(self.inner.index, cx.waker().clone());
        core::mem::drop(wakers_lock);

        let events_lock = self.inner.events.read().unwrap();
        let mut first = events_lock.to_emit.first();

        // ignore events which occur after poll_time. don't ignore rollbacks.
        if let Some(x) = first {
            if let RollbackEvent::Event { time, .. } = x.as_ref() {
                if time > &poll_time {
                    first = None;
                }
            }
        }
        let should_emit_stored = first.is_some();
        core::mem::drop(events_lock);
        
        // be careful with rollbacks. they can't be sorted, so when one is emitted, all the duplicates need to purge cancelled events
        // rollbacks always sort first. Rolled back events are removed immediately when we first receive the rollback from the original
        if should_emit_stored {
            let mut events_lock = self.inner.events.write().unwrap();
            // this works because the only other writers are other duplicates, which can remove events, but not rollbacks.
            // additionally, if an event is removed, a rollback must also be inserted at or before it.
            // we definitely know that whatever is first in this to_emit queue from the last lock is still the thing we want to emit.
            let first = events_lock.to_emit.pop_first().unwrap();
            // try to pull the event out of the arc; clone if there are other references
            let owned = Arc::try_unwrap(first).unwrap_or_else(|a| (*a).clone());
            return owned.into();
        }

        let waker = self.get_waker();
        let mut context = Context::from_waker(&waker);

        let mut source_lock = self.inner.original.source.lock().unwrap();

        // SAFETY: our source is structurally pinned inside a mutex inside original, which is an Arc, therefore unmoving.
        let source: Pin<&mut Src> = unsafe { Pin::new_unchecked(&mut source_lock) };
        let poll = poll_fn(source, poll_time, &mut context);

        // immediately delete all stored events at or after t.
        if let SourcePoll::Rollback(t) = &poll {
            let children_lock = self.inner.original.children.read().unwrap();
            for (_, dup) in children_lock.iter() {
                if let Some(dup) = dup.upgrade() {
                    let key = RollbackEvent::Search { time: *t };
                    let mut events_lock = dup.events.write().unwrap();
                    
                    // throw away all stored events at or after time t
                    core::mem::drop(events_lock.to_emit.split_off(&key));
                    core::mem::drop(events_lock);
                }
            }
        }
        // now we can release the lock.
        core::mem::drop(source_lock);

        // persist the promises we've made about scheduling.
        match &poll {
            SourcePoll::Pending => {},
            SourcePoll::Scheduled(_s, t) => {
                *self.inner.original.scheduled.write().unwrap() = Some(*t);
            },
            _ => {
                *self.inner.original.scheduled.write().unwrap() = None;
            },
        };

        match poll {
            SourcePoll::Pending => SourcePoll::Pending,
            SourcePoll::Rollback(t) => {
                let rollback_event = RollbackEvent::Rollback { time: t };
                let rollback_event = Arc::new(rollback_event);

                let children_lock = self.inner.original.children.read().unwrap();
                for (i, dup) in children_lock.iter() {
                    if *i != self.inner.index {
                        if let Some(dup) = dup.upgrade() {
                            let mut events_lock = dup.events.write().unwrap();
                            events_lock.to_emit.insert(rollback_event.clone());
                            core::mem::drop(events_lock);
                        }
                    }
                }

                core::mem::drop(children_lock);

                SourcePoll::Rollback(t)
            }
            SourcePoll::Event(e, t) => {
                let rollback_event = RollbackEvent::Event {
                    event: e.clone(),
                    time: t,
                };
                let rollback_event = Arc::new(rollback_event);

                let children_lock = self.inner.original.children.read().unwrap();
                for (i, dup) in children_lock.iter() {
                    if *i != self.inner.index {
                        if let Some(dup) = dup.upgrade() {
                            let mut events_lock = dup.events.write().unwrap();
                            events_lock.to_emit.insert(rollback_event.clone());
                            core::mem::drop(events_lock);
                        }
                    }
                }
                core::mem::drop(children_lock);

                SourcePoll::Event(e, t)
            }
            SourcePoll::Scheduled(s, t) => {
                let events_lock = self.inner.events.read().unwrap();

                for e in events_lock.to_emit.iter() {
                    if let RollbackEvent::Event { time, .. } = **e {
                        if time < t {
                            return SourcePoll::Scheduled(s, time);
                        } else {
                            break;
                        }
                    }
                }
                core::mem::drop(events_lock);

                SourcePoll::Scheduled(s, t)
            }
            SourcePoll::Ready(s) => {
                let events_lock = self.inner.events.read().unwrap();

                for e in events_lock.to_emit.iter() {
                    if let RollbackEvent::Event { time, .. } = **e {
                        return SourcePoll::Scheduled(s, time);
                    }
                }
                core::mem::drop(events_lock);

                SourcePoll::Ready(s)
            }
        }
    }
}

impl<Src: Source> Clone for Duplicate<Src>
where
    Src::Event: Clone,
{
    fn clone(&self) -> Self {
        let original = self.inner.original.clone();
        let mut children_lock = self.inner.original.children.write().unwrap();
        let max_index = *children_lock.last_key_value().unwrap().0;

        let index = max_index + 1;
        let inner = DuplicateInner {
            index,
            original,
            events: RwLock::new(Events::new()),
        };
        let inner = Arc::new(inner);
        children_lock.insert(index, Arc::downgrade(&inner));
        core::mem::drop(children_lock);

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
    ) -> SourcePoll<Self::Time, Self::Event, Self::State> {
        self.poll_internal(poll_time, cx, Src::poll)
    }

    fn poll_forget(
        self: Pin<&mut Self>,
        poll_time: Self::Time,
        cx: &mut Context<'_>,
    ) -> SourcePoll<Self::Time, Self::Event, Self::State> {
        self.poll_internal(poll_time, cx, Src::poll_forget)
    }

    fn poll_events(
        self: Pin<&mut Self>,
        poll_time: Self::Time,
        cx: &mut Context<'_>,
    ) -> SourcePoll<Self::Time, Self::Event, ()> {
        self.poll_internal(poll_time, cx, Src::poll_events)
    }
}
