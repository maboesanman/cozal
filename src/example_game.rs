use crate::core::{
    event::event::{Event, RollbackPayload},
    transposer::{
        transposer::{InitResult, Transposer, UpdateResult},
        transposer_context::TransposerContext,
        transposer_event::{InternalTransposerEvent, TransposerEvent},
    },
};
use async_trait::async_trait;
use futures::future::ready;
use futures::{Stream, StreamExt};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct ExampleTransposer {
    count: isize,
}

#[async_trait]
impl Transposer for ExampleTransposer {
    type Time = Duration;
    type External = ();
    type Internal = ();
    type Out = isize;

    async fn init(_cx: &TransposerContext) -> InitResult<Self> {
        InitResult {
            new_updater: ExampleTransposer { count: 0 },
            new_events: vec![Event {
                timestamp: Duration::from_secs(0),
                payload: (),
            }],
            emitted_events: vec![],
        }
    }
    async fn update<'a>(
        &'a self,
        _cx: &TransposerContext,
        // all these events have the same time.
        events: Vec<&TransposerEvent<Self>>,
    ) -> UpdateResult<Self> {
        let mut new_updater = self.clone();
        let mut result = UpdateResult {
            new_updater: None,
            expired_events: Vec::new(),
            new_events: Vec::new(),
            emitted_events: Vec::new(),
            exit: false,
        };
        for event in events {
            match event {
                TransposerEvent::External(_) => {
                    new_updater.count -= 1;
                }
                TransposerEvent::Internal(InternalTransposerEvent { event, .. }) => {
                    new_updater.count += 1;
                    result.new_events.push(Event {
                        timestamp: event.timestamp + Duration::from_secs(1) / 2,
                        payload: (),
                    });
                }
            }
            result.emitted_events.push(new_updater.count);
        }
        if new_updater.count != self.count {
            result.new_updater = Some(new_updater)
        }
        result
    }
}

pub fn get_filtered_stream<S: Stream<Item = Event<Instant, winit::event::Event<'static, ()>>>>(
    start_time: Instant,
    stream: S,
) -> impl Stream<Item = Event<Duration, RollbackPayload<()>>> {
    stream.filter_map(move |e: Event<Instant, winit::event::Event<'_, ()>>| {
        ready(match e.payload {
            winit::event::Event::WindowEvent {
                window_id: _,
                event,
            } => match event {
                winit::event::WindowEvent::ReceivedCharacter(_) => {
                    let event = Event {
                        timestamp: e.timestamp - start_time,
                        payload: RollbackPayload::Payload(()),
                    };
                    Some(event)
                }
                _ => None,
            },
            _ => None,
        })
    })
}
