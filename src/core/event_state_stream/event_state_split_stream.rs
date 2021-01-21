use core::task::{Poll, Context, Waker};
use core::pin::Pin;
use futures::{lock::BiLock, sink::Buffer};
use std::{cmp::{self, Ordering, Reverse}, collections::{BinaryHeap, VecDeque}};
use either::Either;
use pin_project::pin_project;

use super::{EventStatePoll, EventStateStream};

#[pin_project]
struct EventStateSplitInner<S: EventStateStream<Event = Either<L, R>>, L, R> {
    #[pin]
    stream: S,
    
    left: BufferData<S::Time, L, S::State>,
    right: BufferData<S::Time, R, S::State>,
    rough_buffer_size: Option<usize>,
}

struct BufferData<T: Ord + Copy, E, S> {
    buffer: BinaryHeap<Reverse<BufferItem<T, E, S>>>,
    latest_emission_time: Option<T>,
    needs_rollback: Option<T>,
    waker: Option<Waker>,
    event_index: usize,
}

struct BufferItem<T: Ord + Copy, E, S>{
    time: T,
    index: usize,
    event: E,
    state: S,
}

impl<T: Ord + Copy, E, S> Ord for BufferItem<T, E, S> {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.time.cmp(&other.time) {
            Ordering::Equal => self.index.cmp(&other.index),
            ord => ord,
        }
    }
}

impl<T: Ord + Copy, E, S> PartialOrd for BufferItem<T, E, S> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Ord + Copy, E, S> PartialEq for BufferItem<T, E, S> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<T: Ord + Copy, E, S> Eq for BufferItem<T, E, S> {}

impl<T: Ord + Copy, E, S> BufferData<T, E, S> {
    fn new() -> Self {
        Self {
            buffer: BinaryHeap::new(),
            latest_emission_time: None,
            needs_rollback: None,
            waker: None,
            event_index: 0,
        }
    }

    fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: BinaryHeap::with_capacity(capacity),
            latest_emission_time: None,
            needs_rollback: None,
            waker: None,
            event_index: 0,
        }
    }

    fn full(&self, max: Option<usize>) -> bool {
        match max {
            Some(s) => self.buffer.len() >= s,
            None => false,
        }
    }

    fn pop_viable(&mut self, poll_time: T) -> Option<(T, E, S)> {
        match self.buffer.peek() {
            Some(Reverse(item)) => {
                if item.time <= poll_time {
                    let Reverse(BufferItem { time, index: _, event, state }) = self.buffer.pop().unwrap();
                    Some((time, event, state))
                } else {
                    None
                }
            },
            None => None
        }
    }

    fn buffer_event(&mut self, time: T, event: E, state: S) {
        let item = BufferItem {
            time,
            index: self.event_index,
            event,
            state,
        };
        self.event_index += 1;
        self.buffer.push(Reverse(item));
    }

    fn rollback(&mut self, rollback_time: T) {
        self.buffer.retain(|Reverse(item)| item.time < rollback_time);
        if let Some(t) = self.latest_emission_time {
            if t >= rollback_time {
                self.needs_rollback = Some(rollback_time);
            }
        }
        
        for waker in self.waker.take() {
            waker.wake();
        }
    }

    fn next_time(&self) -> Option<T> {
        self.buffer.peek().map(|Reverse(x)| x.time)
    }
}

impl<S: EventStateStream<Event = Either<L, R>>, L, R> EventStateSplitInner<S, L, R> {
    fn new(stream: S, rough_buffer_size: Option<usize>) -> Self {
        let (left, right) = match rough_buffer_size {
            Some(cap) => (BufferData::with_capacity(cap), BufferData::with_capacity(cap)),
            None => (BufferData::new(), BufferData::new())
        };
        Self {
            stream,

            rough_buffer_size,
            left,
            right,
        }
    }

    fn poll_left(
        self: Pin<&mut Self>,
        poll_time: S::Time,
        cx: &mut Context<'_>,
    ) -> EventStatePoll<S::Time, L, S::State> {
        let mut this = self.project();

        loop {
            if let Some(t) = this.left.needs_rollback {
                this.left.needs_rollback = None;
                break EventStatePoll::Rollback(t);
            }
    
            if let Some((time, event, state)) = this.left.pop_viable(poll_time) {
                match this.left.latest_emission_time {
                    Some(old) => this.left.latest_emission_time = Some(cmp::max(old, time)),
                    None => this.left.latest_emission_time = Some(time)
                }
                for waker in this.right.waker.take() {
                    waker.wake();
                }
                
                break EventStatePoll::Event(time, event, state);
            }
    
            if this.right.full(*this.rough_buffer_size) {
                this.left.waker = Some(cx.waker().clone());
                break EventStatePoll::Pending;
            }

            match this.stream.as_mut().poll(poll_time, cx) {
                EventStatePoll::Pending => break EventStatePoll::Pending,
                EventStatePoll::Rollback(t) => {
                    this.right.rollback(t);
                    this.left.rollback(t);
                },
                EventStatePoll::Event(t, Either::Left(e), s) => {
                    match this.left.latest_emission_time {
                        Some(old) => this.left.latest_emission_time = Some(cmp::max(old, t)),
                        None => this.left.latest_emission_time = Some(t)
                    }
                    break EventStatePoll::Event(t, e, s);
                },
                EventStatePoll::Event(t, Either::Right(e), s) => {
                    this.right.buffer_event(t, e, s);
                },
                EventStatePoll::Scheduled(mut t, s) => {
                    if let Some(t_buf) = this.left.next_time() {
                        t = cmp::min(t, t_buf);
                    }
                    break EventStatePoll::Scheduled(t, s);
                },
                EventStatePoll::Ready(s) => {
                    break match this.left.next_time() {
                        Some(t) => EventStatePoll::Scheduled(t, s),
                        None => EventStatePoll::Ready(s),
                    };
                },
                EventStatePoll::Done(s) => {
                    break match this.left.next_time() {
                        Some(t) => EventStatePoll::Scheduled(t, s),
                        None => EventStatePoll::Done(s),
                    };
                },
            };
        }
    }

    fn poll_right(
        self: Pin<&mut Self>,
        poll_time: S::Time,
        cx: &mut Context<'_>,
    ) -> EventStatePoll<S::Time, R, S::State> {
        let mut this = self.project();

        loop {
            if let Some(t) = this.right.needs_rollback {
                this.right.needs_rollback = None;
                break EventStatePoll::Rollback(t);
            }
    
            if let Some((time, event, state)) = this.right.pop_viable(poll_time) {
                match this.right.latest_emission_time {
                    Some(old) => this.right.latest_emission_time = Some(cmp::max(old, time)),
                    None => this.right.latest_emission_time = Some(time)
                }

                for waker in this.left.waker.take() {
                    waker.wake();
                }
                
                break EventStatePoll::Event(time, event, state);
            }
    
            if this.left.full(*this.rough_buffer_size) {
                this.right.waker = Some(cx.waker().clone());
                break EventStatePoll::Pending;
            }

            match this.stream.as_mut().poll(poll_time, cx) {
                EventStatePoll::Pending => break EventStatePoll::Pending,
                EventStatePoll::Rollback(t) => {
                    this.left.rollback(t);
                    this.right.rollback(t);
                },
                EventStatePoll::Event(t, Either::Right(e), s) => {
                    match this.right.latest_emission_time {
                        Some(old) => this.right.latest_emission_time = Some(cmp::max(old, t)),
                        None => this.right.latest_emission_time = Some(t)
                    }
                    break EventStatePoll::Event(t, e, s);
                },
                EventStatePoll::Event(t, Either::Left(e), s) => {
                    this.left.buffer_event(t, e, s);
                },
                EventStatePoll::Scheduled(mut t, s) => {
                    if let Some(t_buf) = this.right.next_time() {
                        t = cmp::min(t, t_buf);
                    }
                    break EventStatePoll::Scheduled(t, s);
                },
                EventStatePoll::Ready(s) => {
                    break match this.right.next_time() {
                        Some(t) => EventStatePoll::Scheduled(t, s),
                        None => EventStatePoll::Ready(s),
                    };
                },
                EventStatePoll::Done(s) => {
                    break match this.right.next_time() {
                        Some(t) => EventStatePoll::Scheduled(t, s),
                        None => EventStatePoll::Done(s),
                    };
                },
            };
        }
    }
}

#[pin_project]
pub struct EventStateSplitLeft<S: EventStateStream<Event = Either<L, R>>, L, R> {
    #[pin]
    inner: BiLock<EventStateSplitInner<S, L, R>>,
}

#[pin_project]
pub struct EventStateSplitRight<S: EventStateStream<Event = Either<L, R>>, L, R> {
    #[pin]
    inner: BiLock<EventStateSplitInner<S, L, R>>,
}

impl<S: EventStateStream<Event = Either<L, R>>, L, R> EventStateStream for EventStateSplitLeft<S, L, R> {
    type Time = S::Time;
    type Event = L;
    type State = S::State;

    fn poll(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: &mut Context<'_>,
    ) -> EventStatePoll<S::Time, L, S::State> {
        let this = Pin::into_inner(self);
        let mut lock = match this.inner.poll_lock(cx) {
            Poll::Pending => return EventStatePoll::Pending,
            Poll::Ready(lock) => lock
        };
        lock.as_pin_mut().poll_left(time, cx)
    }
}

impl<S: EventStateStream<Event = Either<L, R>>, L, R> EventStateStream for EventStateSplitRight<S, L, R> {
    type Time = S::Time;
    type Event = R;
    type State = S::State;

    fn poll(
        self: Pin<&mut Self>,
        time: Self::Time,
        cx: &mut Context<'_>,
    ) -> EventStatePoll<S::Time, R, S::State> {
        let this = Pin::into_inner(self);
        let mut lock = match this.inner.poll_lock(cx) {
            Poll::Pending => return EventStatePoll::Pending,
            Poll::Ready(lock) => lock
        };
        lock.as_pin_mut().poll_right(time, cx)
    }
}

pub fn bounded<S: EventStateStream<Event = Either<L, R>>, L, R>(stream: S, buffer_size: usize) -> (
    EventStateSplitLeft<S, L, R>,
    EventStateSplitRight<S, L, R>,
) {
    let inner = EventStateSplitInner::new(stream, Some(buffer_size));
    let (left, right) = BiLock::new(inner);

    let left = EventStateSplitLeft { inner: left };
    let right = EventStateSplitRight { inner: right };

    (left, right)
}

pub fn unbounded<S: EventStateStream<Event = Either<L, R>>, L, R>(stream: S) -> (
    EventStateSplitLeft<S, L, R>,
    EventStateSplitRight<S, L, R>,
) {
    let inner = EventStateSplitInner::new(stream, None);
    let (left, right) = BiLock::new(inner);

    let left = EventStateSplitLeft { inner: left };
    let right = EventStateSplitRight { inner: right };

    (left, right)
}
