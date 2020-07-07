use crate::core::event_factory::EventFactory;
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
    Init(Pin<Box<dyn Future<Output = GameInitResult<In, Internal, Out, U>>>>),
    Update(Pin<Box<dyn Future<Output = GameUpdateResult<In, Internal, Out, U>>>>),
    Scheduled(Delay),
    NewEvent,
}

pub struct GameInitResult<In: Clone, Internal: Clone, Out: Clone, U: GameUpdater<In, Internal, Out>> {
    pub new_updater: U,
    pub new_events: Vec<ScheduleEvent<In, Internal>>,
    pub emitted_events: Vec<Event<Out>>,
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
    fn init(ef: &'static EventFactory) -> Pin<Box<dyn Future<Output = GameInitResult<In, Internal, Out, Self>>>>;
    fn update(&self, event: ScheduleEvent<In, Internal>, ef: &'static EventFactory) -> Pin<Box<dyn Future<Output = GameUpdateResult<In, Internal, Out, Self>>>>;
}

pub struct Game<
    In: Unpin + Clone,
    Internal: Unpin + Clone,
    Out: Unpin + Clone,
    U: GameUpdater<In, Internal, Out>,
    Err: Unpin + Clone,
> {
    ef: &'static EventFactory,

    // replace this a history of updaters.
    schedule: OrdSet<ScheduleEvent<In, Internal>>,
    updater: Option<U>,

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
    pub fn new(ef: &'static EventFactory) -> Self {
        Game {
            ef,
            schedule: OrdSet::new(),
            updater: None,
            start: Instant::now(),
            last_event: None,
            waiting_for: WaitingFor::Init(U::init(ef)),
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
                        let fut = self.updater.as_ref().unwrap().update(event.unwrap(), self.ef);
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

    fn poll_updater(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        match &mut self.waiting_for {
            WaitingFor::Update(future) => {
                match Pin::new(future).poll(cx) {
                    Poll::Ready(update_result) => {
                        self.updater = Some(update_result.new_updater);
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
                            if e.content.timestamp < t.timestamp() {
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
            WaitingFor::Init(future) => {
                match Pin::new(future).poll(cx) {
                    Poll::Ready(init_result) => {
                        self.updater = Some(init_result.new_updater);
                        for e in init_result.new_events.into_iter() {
                            self.schedule = self.schedule.update(e);
                        }
                        for e in init_result.emitted_events.into_iter() {
                            self.output_buffer.push_back(e);
                        }
                        self.waiting_for = self.get_waiting_for_from_schedule();

                        Poll::Ready(())
                    },
                    Poll::Pending => {
                        Poll::Pending
                    }
                }
            }
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
    type Item = Result<Event<Out>, Err>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let self_mut = self.get_mut();
        loop {
            match self_mut.output_buffer.pop_front() {
                Some(event) => break Poll::Ready(Some(Ok(event))),
                None => match self_mut.waiting_for {
                    WaitingFor::Init(_) | WaitingFor::Update(_) => {
                        match self_mut.poll_updater(cx) {
                            Poll::Ready(_) => {},
                            Poll::Pending => break Poll::Pending,
                        }
                    }
                    WaitingFor::Scheduled(_) => {
                        match self_mut.poll_schedule(cx) {
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
        match self.get_mut().poll_updater(cx) {
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
