use crate::core::{
    event::event::{Event, EventPayload, EventTimestamp},
    transposer::{
        transposer::{InitResult, Transposer, UpdateResult},
        transposer_context::TransposerContext,
        trigger_event::TriggerEvent,
    },
};
use async_trait::async_trait;
use std::time::Duration;
use futures::{StreamExt, Stream};
use futures::future::ready;

#[derive(Clone)]
pub struct ExampleTransposer {
    count: usize,
}

impl ExampleTransposer {
    fn handle_external(&self, event: &Event<()>) -> UpdateResult<Self> {
        let mut new_updater = self.clone();
        new_updater.count -= 1;
        let new_out_event = Event {
            timestamp: EventTimestamp {
                time: event.timestamp.time + Duration::from_secs(1),
                priority: 0,
            },
            payload: EventPayload::Payload(self.count),
        };
        UpdateResult {
            new_updater: Some(new_updater),
            expired_events: vec![],
            new_events: vec![],
            emitted_events: vec![new_out_event],
        }
    }

    fn handle_internal(&self, event: &Event<()>) -> UpdateResult<Self> {
        let mut new_updater = self.clone();
        new_updater.count += 1;
        let new_in_event = Event {
            timestamp: EventTimestamp {
                time: event.timestamp.time + Duration::from_secs(1),
                priority: 0,
            },
            payload: EventPayload::Payload(()),
        };
        let new_out_event = Event {
            timestamp: EventTimestamp {
                time: event.timestamp.time + Duration::from_secs(1),
                priority: 0,
            },
            payload: EventPayload::Payload(self.count),
        };
        UpdateResult {
            new_updater: Some(new_updater),
            expired_events: vec![],
            new_events: vec![new_in_event],
            emitted_events: vec![new_out_event],
        }
    }
}

#[async_trait]
impl Transposer for ExampleTransposer {
    type External = ();
    type Internal = ();
    type Out = usize;

    async fn init(_cx: &TransposerContext) -> InitResult<Self> {
        InitResult {
            new_updater: ExampleTransposer { count: 0 },
            new_events: vec![Event {
                timestamp: EventTimestamp {
                    time: Duration::from_secs(0),
                    priority: 0,
                },
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

pub fn get_filtered_stream<S: Stream<Item = Event<winit::event::Event<'static, ()>>>>(
    stream: S,
) -> impl Stream<Item = Event<()>> {
    stream.filter_map(
        move |e: Event<winit::event::Event<'_, ()>>| ready(match e.payload {
            EventPayload::Payload(winit::event::Event::WindowEvent {
                window_id: _,
                event,
            }) => match event {
                winit::event::WindowEvent::ReceivedCharacter(_) => {
                    let event = Event {
                        timestamp: e.timestamp,
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
