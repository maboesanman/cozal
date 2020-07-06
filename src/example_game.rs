
// use crate::core::event::EventTimestamp;
use crate::core::event::{Event, EventTimestamp, EventPayload};
use futures::{Future, FutureExt};
use std::pin::Pin;
use crate::core::event::ScheduleEvent;
use crate::core::game::{GameUpdater, GameUpdateResult};
use std::time::Duration;

#[derive(Clone)]
pub struct MyUpdater {
    count: usize
}

impl MyUpdater {
    async fn initialize_internal() -> MyUpdater {
        
    }
    async fn update_internal(mut self, event: ScheduleEvent<(), ()>) -> GameUpdateResult<(), (), usize, Self> {
        let mut new_events = vec![];
        let mut emitted_events = vec![];
        match event {
            ScheduleEvent::External(_) => self.count -= 1,
            ScheduleEvent::Internal(_) => {
                self.count += 1;
                let new_in_event = ScheduleEvent::Internal(Event {
                    id: self.count,
                    timestamp: EventTimestamp {
                        time: event.timestamp().time + Duration::from_secs(1),
                        priority: 1,
                    },
                    payload: EventPayload::Custom(()),
                });
                new_events = vec![new_in_event];
                let new_out_event = Event {
                    id: self.count,
                    timestamp: EventTimestamp {
                        time: event.timestamp().time + Duration::from_secs(1),
                        priority: 1,
                    },
                    payload: EventPayload::Custom(self.count),
                };
                emitted_events = vec![new_out_event];
            },
        };
        GameUpdateResult {
            new_updater: self,
            trigger: event.clone(),
            expired_events: vec![],
            new_events,
            emitted_events,
        }
    }
}

impl GameUpdater<(), (), usize> for MyUpdater {
    fn update(&self, event: ScheduleEvent<(), ()>) -> Pin<Box<dyn Future<Output = GameUpdateResult<(), (), usize, Self>>>> {
        let new_updater = self.clone();
        async move {
            new_updater.update_internal(event).await
        }.boxed()
    }
}