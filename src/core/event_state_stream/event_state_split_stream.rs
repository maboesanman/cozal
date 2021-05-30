use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use either::Either;
use futures::lock::BiLock;
use pin_project::pin_project;
use std::{
    cmp::{self, Ordering, Reverse},
    collections::BinaryHeap,
};

use super::{EventStatePoll, EventStateStream};

#[pin_project]
struct EventStateSplitInner<S: EventStateStream<Event = Either<L, R>>, L, R> {
    #[pin]
    stream: S,

    left: BufferData<S::Time, L>,
    right: BufferData<S::Time, R>,
    rough_buffer_size: Option<usize>,
}

struct BufferData<T: Ord + Copy, E> {
    buffer: BinaryHeap<Reverse<BufferItem<T, E>>>,
    latest_emission_time: Option<T>,
    needs_rollback: Option<T>,
    waker: Option<Waker>,
    event_index: usize,
}

struct BufferItem<T: Ord + Copy, E> {
    time: T,
    index: usize,
    event: E,
}

impl<T: Ord + Copy, E> Ord for BufferItem<T, E> {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.time.cmp(&other.time) {
            Ordering::Equal => self.index.cmp(&other.index),
            ord => ord,
        }
    }
}

impl<T: Ord + Copy, E> PartialOrd for BufferItem<T, E> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Ord + Copy, E> PartialEq for BufferItem<T, E> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<T: Ord + Copy, E> Eq for BufferItem<T, E> {}

impl<T: Ord + Copy, E> BufferData<T, E> {
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

    fn pop_viable(&mut self, poll_time: T) -> Option<(T, E)> {
        match self.buffer.peek() {
            Some(Reverse(item)) => {
                if item.time <= poll_time {
                    let Reverse(BufferItem {
                        time,
                        index: _,
                        event,
                    }) = self.buffer.pop().unwrap();
                    Some((time, event))
                } else {
                    None
                }
            }
            None => None,
        }
    }

    fn buffer_event(&mut self, time: T, event: E) {
        let item = BufferItem {
            time,
            index: self.event_index,
            event,
        };
        self.event_index += 1;
        self.buffer.push(Reverse(item));
    }

    fn rollback(&mut self, rollback_time: T) {
        self.buffer
            .retain(|Reverse(item)| item.time < rollback_time);
        if let Some(t) = self.latest_emission_time {
            if t >= rollback_time {
                self.needs_rollback = Some(rollback_time);
            }
        }

        if let Some(waker) = self.waker.take() {
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
            Some(cap) => (
                BufferData::with_capacity(cap),
                BufferData::with_capacity(cap),
            ),
            None => (BufferData::new(), BufferData::new()),
        };
        Self {
            stream,

            left,
            right,
            rough_buffer_size,
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

            if let Some((time, event)) = this.left.pop_viable(poll_time) {
                match this.left.latest_emission_time {
                    Some(old) => this.left.latest_emission_time = Some(cmp::max(old, time)),
                    None => this.left.latest_emission_time = Some(time),
                }
                if let Some(waker) = this.right.waker.take() {
                    waker.wake();
                }

                break EventStatePoll::Event(time, event);
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
                }
                EventStatePoll::Event(t, Either::Left(e)) => {
                    match this.left.latest_emission_time {
                        Some(old) => this.left.latest_emission_time = Some(cmp::max(old, t)),
                        None => this.left.latest_emission_time = Some(t),
                    }
                    break EventStatePoll::Event(t, e);
                }
                EventStatePoll::Event(t, Either::Right(e)) => {
                    this.right.buffer_event(t, e);
                }
                EventStatePoll::Scheduled(mut t, s) => {
                    if let Some(t_buf) = this.left.next_time() {
                        t = cmp::min(t, t_buf);
                    }
                    break EventStatePoll::Scheduled(t, s);
                }
                EventStatePoll::Ready(s) => {
                    break match this.left.next_time() {
                        Some(t) => EventStatePoll::Scheduled(t, s),
                        None => EventStatePoll::Ready(s),
                    };
                }
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

            if let Some((time, event)) = this.right.pop_viable(poll_time) {
                match this.right.latest_emission_time {
                    Some(old) => this.right.latest_emission_time = Some(cmp::max(old, time)),
                    None => this.right.latest_emission_time = Some(time),
                }

                if let Some(waker) = this.left.waker.take() {
                    waker.wake();
                }

                break EventStatePoll::Event(time, event);
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
                }
                EventStatePoll::Event(t, Either::Right(e)) => {
                    match this.right.latest_emission_time {
                        Some(old) => this.right.latest_emission_time = Some(cmp::max(old, t)),
                        None => this.right.latest_emission_time = Some(t),
                    }
                    break EventStatePoll::Event(t, e);
                }
                EventStatePoll::Event(t, Either::Left(e)) => {
                    this.left.buffer_event(t, e);
                }
                EventStatePoll::Scheduled(mut t, s) => {
                    if let Some(t_buf) = this.right.next_time() {
                        t = cmp::min(t, t_buf);
                    }
                    break EventStatePoll::Scheduled(t, s);
                }
                EventStatePoll::Ready(s) => {
                    break match this.right.next_time() {
                        Some(t) => EventStatePoll::Scheduled(t, s),
                        None => EventStatePoll::Ready(s),
                    };
                }
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

impl<S: EventStateStream<Event = Either<L, R>>, L, R> EventStateStream
    for EventStateSplitLeft<S, L, R>
{
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
            Poll::Ready(lock) => lock,
        };
        lock.as_pin_mut().poll_left(time, cx)
    }
}

impl<S: EventStateStream<Event = Either<L, R>>, L, R> EventStateStream
    for EventStateSplitRight<S, L, R>
{
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
            Poll::Ready(lock) => lock,
        };
        lock.as_pin_mut().poll_right(time, cx)
    }
}

pub fn bounded<S: EventStateStream<Event = Either<L, R>>, L, R>(
    stream: S,
    buffer_size: usize,
) -> (EventStateSplitLeft<S, L, R>, EventStateSplitRight<S, L, R>) {
    let inner = EventStateSplitInner::new(stream, Some(buffer_size));
    let (left, right) = BiLock::new(inner);

    let left = EventStateSplitLeft { inner: left };
    let right = EventStateSplitRight { inner: right };

    (left, right)
}

pub fn unbounded<S: EventStateStream<Event = Either<L, R>>, L, R>(
    stream: S,
) -> (EventStateSplitLeft<S, L, R>, EventStateSplitRight<S, L, R>) {
    let inner = EventStateSplitInner::new(stream, None);
    let (left, right) = BiLock::new(inner);

    let left = EventStateSplitLeft { inner: left };
    let right = EventStateSplitRight { inner: right };

    (left, right)
}
