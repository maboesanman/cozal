use crate::core::event_factory::EventFactory;
use std::collections::VecDeque;
use core::marker::PhantomData;
use core::pin::Pin;
use futures::{Future, Stream};
use futures::task::{Poll, Context};
use tokio::time::{delay_until, Delay, Instant};
use im::OrdSet;

use super::{
    updater::{InitResult, Updater, UpdateResult},
    event::{
        Event, 
        ScheduleEvent
    }
};

enum WaitingFor<U: Updater> {
    Init(Pin<Box<dyn Future<Output = InitResult<U>>>>),
    Update(Pin<Box<dyn Future<Output = UpdateResult<U>>>>),
    Scheduled(Delay),
    NewEvent,
}

pub struct Game<
    U: Updater,
    S: Stream<Item = Event<U::In>> + Unpin + Send,
    Err: Unpin + Clone + Send,
> {
    ef: &'static EventFactory,
    input_stream: S,

    // replace this a history of updaters.
    schedule: OrdSet<ScheduleEvent<U::In, U::Internal>>,
    updater: Option<U>,

    start: Instant,
    last_event: Option<ScheduleEvent<U::In, U::Internal>>,
    waiting_for: WaitingFor<U>,
    output_buffer: VecDeque<Event<U::Out>>,

    phantom: PhantomData<Err>,
}

impl<
    U: Updater,
    S: Stream<Item = Event<U::In>> + Unpin + Send,
    Err: Unpin + Clone + Send,
> Game<U, S, Err> {
    pub fn new(input_stream: S, ef: &'static EventFactory) -> Self {
        Game {
            ef,
            input_stream: input_stream,
            schedule: OrdSet::new(),
            updater: None,
            start: Instant::now(),
            last_event: None,
            waiting_for: WaitingFor::Init(U::init(ef)),
            output_buffer: VecDeque::new(),
            phantom: PhantomData,
        }
    }
    fn get_waiting_for_from_schedule(&self) -> WaitingFor<U> {
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
    U: Updater,
    S: Stream<Item = Event<U::In>> + Unpin + Send,
    Err: Unpin + Clone + Send,
> Stream for Game<U, S, Err> {
    type Item = Result<Event<U::Out>, Err>;

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
