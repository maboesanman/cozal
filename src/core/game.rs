use std::collections::VecDeque;
use core::marker::PhantomData;
use core::pin::Pin;
use futures::{Future, Sink, Stream};
use futures::task::{Poll, Context};
use tokio::time::{delay_until, Delay, Instant};
use im::OrdSet;

use super::event::{Event, ScheduleEvent};

enum WaitingFor<
    In: Unpin + Clone,
    Internal: Unpin + Clone,
    Out: Unpin + Clone,
    U: GameUpdater<In, Internal, Out>,
> {
    Scheduled(Delay),
    Update(Pin<Box<dyn Future<Output = GameUpdateResult<In, Internal, Out, U>>>>),
    NewEvent,
}

pub struct GameUpdateResult<In: Clone, Internal: Clone, Out: Clone, U: GameUpdater<In, Internal, Out>> {
    pub new_updater: U,
    pub trigger: ScheduleEvent<In, Internal>,
    // all these events must be in the future
    pub expired_events: Vec<ScheduleEvent<In, Internal>>,
    pub new_events: Vec<ScheduleEvent<In, Internal>>,
    pub emitted_events: Vec<Event<Out>>,
}

pub trait GameUpdater<In: Clone, Internal: Clone, Out: Clone>: Clone + Unpin {
    fn update(&self, event: ScheduleEvent<In, Internal>) -> Pin<Box<dyn Future<Output = GameUpdateResult<In, Internal, Out, Self>>>>;
}

pub struct Game<
    In: Unpin + Clone,
    Internal: Unpin + Clone,
    Out: Unpin + Clone,
    U: GameUpdater<In, Internal, Out>,
    Err: Unpin + Clone,
> {
    id: usize,
    schedule: OrdSet<ScheduleEvent<In, Internal>>,

    // replace this a history of updaters.
    updater: U,

    start: Instant,
    last_event: Option<ScheduleEvent<In, Internal>>,
    waiting_for: WaitingFor<In, Internal, Out, U>,
    output_buffer: VecDeque<Event<Out>>,

    phantom: PhantomData<Err>,
}

impl<
    In: Unpin + Clone,
    Internal: Unpin + Clone,
    Out: Unpin + Clone,
    U: GameUpdater<In, Internal, Out>,
    Err: Unpin + Clone
> Game<In, Internal, Out, U, Err> {
    fn new(updater: U) -> Self {
        Game {
            id: 0,
            schedule: OrdSet::new(),
            updater,
            start: Instant::now(),
            last_event: None,
            waiting_for: WaitingFor::NewEvent,
            output_buffer: VecDeque::new(),
            phantom: PhantomData,
        }
    }
    fn get_waiting_for_from_schedule(&self) -> WaitingFor<In, Internal, Out, U> {
        match self.schedule.get_min() {
            Some(event) => {
                let time = self.start + event.timestamp().time;
                WaitingFor::Scheduled(delay_until(time))
            },
            None => {
                WaitingFor::NewEvent
            }
        }
    }

    fn poll_schedule(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        match &mut self.waiting_for {
            WaitingFor::Scheduled(delay) => {
                match Pin::new(delay).poll(cx) {
                    Poll::Ready(_) => {
                        let (event, new_schedule) = self.schedule.without_min();
                        self.schedule = new_schedule;
                        let fut = self.updater.update(event.unwrap());
                        self.waiting_for = WaitingFor::Update(fut);
                        Poll::Ready(())
                    },
                    Poll::Pending => {
                        Poll::Pending
                    },
                }
            },
            _ => Poll::Ready(())
        }
    }

    fn poll_update(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        match &mut self.waiting_for {
            WaitingFor::Update(future) => {
                match Pin::new(future).poll(cx) {
                    Poll::Ready(update_result) => {
                        self.updater = update_result.new_updater;
                        let t = update_result.trigger;
                        for e in update_result.expired_events.iter() {
                            if e < &t {
                                panic!()
                            }
                            self.schedule = self.schedule.without(e);
                        }
                        for e in update_result.new_events.into_iter() {
                            if e < t {
                                panic!()
                            }
                            self.schedule = self.schedule.update(e);
                        }
                        for e in update_result.emitted_events.into_iter() {
                            if e.timestamp < t.timestamp() {
                                panic!()
                            }
                            self.output_buffer.push_back(e);
                        }
                        self.last_event = Some(t);
                        self.waiting_for = self.get_waiting_for_from_schedule();

                        Poll::Ready(())
                    },
                    Poll::Pending => {
                        Poll::Pending
                    },
                }
            },
            _ => Poll::Ready(())
        }
    }
}

impl<
    In: Unpin + Clone,
    Internal: Unpin + Clone,
    Out: Unpin + Clone,
    U: GameUpdater<In, Internal, Out>,
    Err: Unpin + Clone
> Stream for Game<In, Internal, Out, U, Err> {
    type Item = Event<Out>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Event<Out>>> {
        let self_mut = self.get_mut();
        loop {
            match self_mut.output_buffer.pop_front() {
                Some(event) => break Poll::Ready(Some(event)),
                None => match self_mut.waiting_for {
                    WaitingFor::Scheduled(_) => {
                        match self_mut.poll_schedule(cx) {
                            Poll::Ready(_) => {},
                            Poll::Pending => break Poll::Pending,
                        }
                    }
                    WaitingFor::Update(_) => {
                        match self_mut.poll_update(cx) {
                            Poll::Ready(_) => {},
                            Poll::Pending => break Poll::Pending,
                        }
                    }
                    WaitingFor::NewEvent => {
                        break Poll::Pending
                    }
                }
            }
        }
    }
}

impl<
    In: Unpin + Clone,
    Internal: Unpin + Clone,
    Out: Unpin + Clone,
    U: GameUpdater<In, Internal, Out>,
    Err: Unpin + Clone
> Sink<Event<In>> for Game<In, Internal, Out, U, Err> {
    type Error = Err;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Err>> {
        match self.get_mut().poll_update(cx) {
            Poll::Ready(_) => Poll::Ready(Ok(())),
            Poll::Pending => Poll::Pending
        }
    }

    fn start_send(self: Pin<&mut Self>, item: Event<In>) -> Result<(), Err> {
        // here is where rollbacks happen
        self.get_mut().schedule.insert(ScheduleEvent::External(item));
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), Err>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), Err>> {
        Poll::Ready(Ok(()))
    }
}
