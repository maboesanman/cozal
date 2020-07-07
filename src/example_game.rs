
// use crate::core::event::EventTimestamp;
use crate::core::event_factory::EventFactory;
use crate::core::event::{EventTimestamp, EventPayload, ScheduleEvent, EventContent};
use crate::core::game::{GameUpdater, GameInitResult, GameUpdateResult};
use futures::{Future, FutureExt};
use std::pin::Pin;
use std::time::Duration;

#[derive(Clone)]
pub struct MyUpdater {
    count: usize
}

impl MyUpdater {
    async fn initialize_internal(ef: &'static EventFactory) -> GameInitResult<(), (), usize, Self> {
        GameInitResult {
            new_updater: MyUpdater { count: 0 },
            new_events: vec![ScheduleEvent::Internal(ef.new_event(EventContent {
                    timestamp: EventTimestamp {
                        time: Duration::from_secs(0),
                        priority: 1,
                    },
                    payload: EventPayload::Custom(()),
                }))],
            emitted_events: vec![],
        }
    }
    async fn update_internal(mut self, event: ScheduleEvent<(), ()>, ef: &'static EventFactory) -> GameUpdateResult<(), (), usize, Self> {
        let mut new_events = vec![];
        let mut emitted_events = vec![];
        match event {
            ScheduleEvent::External(_) => self.count -= 1,
            ScheduleEvent::Internal(_) => {
                self.count += 1;
                let new_in_event = ScheduleEvent::Internal(ef.new_event(EventContent {
                    timestamp: EventTimestamp {
                        time: event.timestamp().time + Duration::from_secs(1),
                        priority: 1,
                    },
                    payload: EventPayload::Custom(()),
                }));
                new_events = vec![new_in_event];
                let new_out_event = ef.new_event(EventContent {
                    timestamp: EventTimestamp {
                        time: event.timestamp().time + Duration::from_secs(1),
                        priority: 1,
                    },
                    payload: EventPayload::Custom(self.count),
                });
                emitted_events = vec![new_out_event];
            },
        };
        GameUpdateResult {
            new_updater: self,
            trigger: event,
            expired_events: vec![],
            new_events,
            emitted_events,
        }
    }
}

impl GameUpdater<(), (), usize> for MyUpdater {
    fn init(ef: &'static EventFactory) -> Pin<Box<dyn Future<Output = GameInitResult<(), (), usize, Self>>>> {
        async move {
            Self::initialize_internal(ef).await
        }.boxed()
    }
    fn update(&self, event: ScheduleEvent<(), ()>, ef: &'static EventFactory) -> Pin<Box<dyn Future<Output = GameUpdateResult<(), (), usize, Self>>>> {
        let new_updater = self.clone();
        async move {
            new_updater.update_internal(event, ef).await
        }.boxed()
    }
}