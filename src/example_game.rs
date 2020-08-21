use crate::core::{
    event::event::{Event, EventPayload},
    transposer::{
        transposer::{InitResult, Transposer, UpdateResult},
        transposer_context::TransposerContext,
        trigger_event::TriggerEvent,
    },
};
use async_trait::async_trait;
use std::time::{Instant, Duration};
use futures::{StreamExt, Stream};
use futures::future::ready;

#[derive(Clone)]
pub struct ExampleTransposer {
    count: usize,
}

impl ExampleTransposer {
    fn handle_external(&self, _event: &Event<Duration, ()>) -> UpdateResult<Self> {
        let mut new_updater = self.clone();
        new_updater.count -= 1;
        UpdateResult {
            new_updater: Some(new_updater),
            expired_events: vec![],
            new_events: vec![],
            emitted_events: vec![self.count],
            rollback: false,
        }
    }

    fn handle_internal(&self, event: &Event<Duration, ()>) -> UpdateResult<Self> {
        let mut new_updater = self.clone();
        new_updater.count += 1;
        let new_in_event = Event {
            timestamp: event.timestamp + Duration::from_secs(1),
            payload: EventPayload::Payload(()),
        };
        UpdateResult {
            new_updater: Some(new_updater),
            expired_events: vec![],
            new_events: vec![new_in_event],
            emitted_events: vec![self.count],
            rollback: false,
        }
    }
}

#[async_trait]
impl Transposer for ExampleTransposer {
    type Time = Duration;
    type External = ();
    type Internal = ();
    type Out = usize;

    async fn init(_cx: &TransposerContext) -> InitResult<Self> {
        InitResult {
            new_updater: ExampleTransposer { count: 0 },
            new_events: vec![Event {
                timestamp: Duration::from_secs(0),
                payload: EventPayload::Payload(()),
            }],
            emitted_events: vec![],
        }
    }
    async fn update<'a>(
        &'a self,
        _cx: &TransposerContext,
        event: &'a TriggerEvent<Self>,
    ) -> UpdateResult<Self> {
        match event {
            TriggerEvent::External(event) => self.handle_external(event),
            TriggerEvent::Internal(event) => self.handle_internal(event),
        }
    }
}

pub fn get_filtered_stream<S: Stream<Item = Event<Instant, winit::event::Event<'static, ()>>>>(
    start_time: Instant,
    stream: S,
) -> impl Stream<Item = Event<Duration, ()>> {
    stream.filter_map(
        move |e: Event<Instant, winit::event::Event<'_, ()>>| ready(match e.payload {
            EventPayload::Payload(winit::event::Event::WindowEvent {
                window_id: _,
                event,
            }) => match event {
                winit::event::WindowEvent::ReceivedCharacter(_) => {
                    let event = Event {
                        timestamp: e.timestamp - start_time,
                        payload: EventPayload::Payload(()),
                    };
                    Some(event)
                }
                _ => None,
            },
            _ => None,
        }),
    )
}
