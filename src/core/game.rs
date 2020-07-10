use crate::core::event_factory::EventFactory;
use core::pin::Pin;
use futures::{
    stream::Fuse,
    task::{Context, Poll},
    StreamExt,
};
use futures::{Future, Stream};
use im::OrdSet;
use std::collections::VecDeque;
use tokio::time::{delay_until, Delay, Instant};

use super::{
    event::{Event, EventContent, ScheduleEvent},
    updater::{InitResult, UpdateResult, Updater},
};

enum WaitingFor<U: Updater> {
    Init(Pin<Box<dyn Future<Output = InitResult<U>>>>),
    Update(Pin<Box<dyn Future<Output = UpdateResult<U>>>>),
    Scheduled(Delay),
    NewEvent,
}

pub struct Game<U: Updater, S: Stream<Item = Event<U::In>> + Unpin + Send> {
    ef: &'static EventFactory,
    input_stream: Fuse<S>,

    // replace this a history of updaters.
    schedule: OrdSet<ScheduleEvent<U::In, U::Internal>>,
    updater: Option<U>,

    start: Instant,
    last_event: Option<ScheduleEvent<U::In, U::Internal>>,
    waiting_for: WaitingFor<U>,
    output_buffer: VecDeque<Event<U::Out>>,
}

impl<U: Updater, S: Stream<Item = Event<U::In>> + Unpin + Send> Game<U, S> {
    pub fn new(input_stream: S, ef: &'static EventFactory) -> Self {
        Game {
            ef,
            input_stream: input_stream.fuse(),
            schedule: OrdSet::new(),
            updater: None,
            start: Instant::now(),
            last_event: None,
            waiting_for: WaitingFor::Init(U::init()),
            output_buffer: VecDeque::new(),
        }
    }
    fn get_waiting_for_from_schedule(&self) -> WaitingFor<U> {
        match self.schedule.get_min() {
            Some(event) => {
                let time = self.start + event.timestamp().time;
                WaitingFor::Scheduled(delay_until(time))
            }
            None => WaitingFor::NewEvent,
        }
    }

    fn schedule_event(&mut self, event: EventContent<U::Internal>) {
        let event = self.ef.new_event(event);
        let event = ScheduleEvent::Internal(event);
        self.schedule = self.schedule.update(event);
    }

    fn emit_event(&mut self, event: EventContent<U::Out>) {
        let event = self.ef.new_event(event);
        self.output_buffer.push_back(event);
    }

    fn poll_input_stream(&mut self, cx: &mut Context<'_>) {
        // do rollback here.
        if self.input_stream.is_done() {
            return;
        }
        loop {
            match Pin::new(&mut self.input_stream).poll_next(cx) {
                Poll::Ready(event) => match event {
                    Some(event) => {
                        let event = ScheduleEvent::External(event);
                        // println!("{:?}", event);
                        self.schedule = self.schedule.update(event);
                        match self.waiting_for {
                            WaitingFor::Init(_) | WaitingFor::Update(_) => {}
                            WaitingFor::Scheduled(_) | WaitingFor::NewEvent => {
                                self.waiting_for = self.get_waiting_for_from_schedule();
                            }
                        }
                    }
                    None => break,
                },
                Poll::Pending => break,
            }
        }
    }

    fn poll_schedule(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        match &mut self.waiting_for {
            WaitingFor::Scheduled(delay) => match Pin::new(delay).poll(cx) {
                Poll::Ready(_) => {
                    let (event, new_schedule) = self.schedule.without_min();
                    self.schedule = new_schedule;
                    let fut = self.updater.as_ref().unwrap().update(event.unwrap());
                    self.waiting_for = WaitingFor::Update(fut);
                    Poll::Ready(())
                }
                Poll::Pending => Poll::Pending,
            },
            _ => Poll::Ready(()),
        }
    }

    fn poll_updater(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        match &mut self.waiting_for {
            WaitingFor::Update(future) => {
                match Pin::new(future).poll(cx) {
                    Poll::Ready(update_result) => {
                        self.updater = Some(update_result.new_updater);
                        let trigger = update_result.trigger;
                        for _ in update_result.expired_events.iter() {
                            todo!()
                            // if e.timestamp() < &t {
                            //     panic!()
                            // }
                            // self.schedule.
                            // self.schedule = self.schedule.without(e);
                        }
                        for e in update_result.new_events.into_iter() {
                            if e.timestamp < trigger.timestamp() {
                                panic!()
                            }
                            self.schedule_event(e);
                        }
                        for e in update_result.emitted_events.into_iter() {
                            if e.timestamp < trigger.timestamp() {
                                panic!()
                            }
                            self.emit_event(e);
                        }
                        self.last_event = Some(trigger);
                        self.waiting_for = self.get_waiting_for_from_schedule();

                        Poll::Ready(())
                    }
                    Poll::Pending => Poll::Pending,
                }
            }
            WaitingFor::Init(future) => match Pin::new(future).poll(cx) {
                Poll::Ready(init_result) => {
                    self.updater = Some(init_result.new_updater);
                    for e in init_result.new_events.into_iter() {
                        self.schedule_event(e);
                    }
                    for e in init_result.emitted_events.into_iter() {
                        self.emit_event(e);
                    }
                    self.waiting_for = self.get_waiting_for_from_schedule();

                    Poll::Ready(())
                }
                Poll::Pending => Poll::Pending,
            },
            _ => Poll::Ready(()),
        }
    }
}

impl<U: Updater, S: Stream<Item = Event<U::In>> + Unpin + Send> Stream for Game<U, S> {
    type Item = Event<U::Out>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let self_mut = self.get_mut();
        self_mut.poll_input_stream(cx);
        loop {
            match self_mut.output_buffer.pop_front() {
                Some(event) => break Poll::Ready(Some(event)),
                None => match self_mut.waiting_for {
                    WaitingFor::Init(_) | WaitingFor::Update(_) => {
                        match self_mut.poll_updater(cx) {
                            Poll::Ready(_) => {}
                            Poll::Pending => break Poll::Pending,
                        }
                    }
                    WaitingFor::Scheduled(_) => match self_mut.poll_schedule(cx) {
                        Poll::Ready(_) => {}
                        Poll::Pending => break Poll::Pending,
                    },
                    WaitingFor::NewEvent => break Poll::Pending,
                },
            }
        }
    }
}
