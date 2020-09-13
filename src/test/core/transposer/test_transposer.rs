use crate::core::transposer::ScheduledEvent;
use crate::core::Transposer;
use crate::core::{
    event::RollbackPayload,
    transposer::{InitContext, UpdateContext},
    Event,
};
use async_trait::async_trait;
use futures::Stream;
use std::task::Poll;

pub(crate) struct EmptyStream {}

impl Stream for EmptyStream {
    type Item = Event<usize, RollbackPayload<usize>>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        Poll::Pending
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum EventCall {
    Input(usize),
    Scheduled(usize),
}

#[derive(Clone)]
pub(crate) struct TestTransposer {
    init_events: Vec<ScheduledEvent<Self>>,
    event_calls: Vec<EventCall>,
}

impl TestTransposer {
    pub fn new(init_events: Vec<ScheduledEvent<Self>>) -> Self {
        Self {
            init_events,
            event_calls: Vec::new(),
        }
    }
}

#[async_trait]
impl Transposer for TestTransposer {
    type Time = usize;

    type Input = usize;

    type Scheduled = usize;

    type Output = Vec<EventCall>;

    async fn init_events(&mut self, cx: &InitContext<Self>) {
        for event in self.init_events.iter() {
            let _ = cx.schedule_event(event.timestamp, event.payload);
        }
    }

    async fn handle_input(
        &mut self,
        time: Self::Time,
        inputs: &[Self::Input],
        cx: &UpdateContext<Self>,
    ) {
        for payload in inputs {
            self.event_calls.push(EventCall::Input(*payload));
            if payload % 2 == 1 {
                let _ = cx.schedule_event(time * 2, payload * 2);
            };
        }
        cx.emit_event(self.event_calls.clone());
    }

    async fn handle_scheduled(
        &mut self,
        time: Self::Time,
        payload: &Self::Scheduled,
        cx: &UpdateContext<Self>,
    ) {
        self.event_calls.push(EventCall::Scheduled(*payload));
        if payload % 2 == 1 {
            let _ = cx.schedule_event(time * 2, payload * 2);
        };
        cx.emit_event(self.event_calls.clone());
    }

    fn can_handle(_event: &crate::core::transposer::InputEvent<Self>) -> bool {
        true
    }
}
