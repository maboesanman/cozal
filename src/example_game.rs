
// use crate::core::event::EventTimestamp;
use crate::core::event_factory::EventFactory;
use crate::core::event::{EventTimestamp, ScheduleEvent, EventContent};
use crate::core::updater::{Updater, InitResult, UpdateResult};
use futures::{Future, FutureExt};
use std::pin::Pin;
use std::time::Duration;

#[derive(Clone)]
pub struct MyUpdater {
    count: usize
}

impl MyUpdater {
    async fn initialize_internal(ef: &'static EventFactory) -> InitResult<Self> {
        InitResult {
            new_updater: MyUpdater { count: 0 },
            new_events: vec![EventContent {
                    timestamp: EventTimestamp {
                        time: Duration::from_secs(0),
                        priority: 1,
                    },
                    payload: (),
                }],
            emitted_events: vec![],
        }
    }
    async fn update_internal(mut self, event: ScheduleEvent<(), ()>, ef: &'static EventFactory) -> UpdateResult<Self> {
        let mut new_events = vec![];
        let mut emitted_events = vec![];
        match event {
            ScheduleEvent::External(_) => self.count -= 1,
            ScheduleEvent::Internal(_) => {
                self.count += 1;
                let new_in_event = EventContent {
                    timestamp: EventTimestamp {
                        time: event.timestamp().time + Duration::from_secs(1),
                        priority: 1,
                    },
                    payload: (),
                };
                new_events = vec![new_in_event];
                let new_out_event = EventContent {
                    timestamp: EventTimestamp {
                        time: event.timestamp().time + Duration::from_secs(1),
                        priority: 1,
                    },
                    payload: self.count,
                };
                emitted_events = vec![new_out_event];
            },
        };
        UpdateResult {
            new_updater: self,
            trigger: event,
            expired_events: vec![],
            new_events,
            emitted_events,
        }
    }
}

impl Updater for MyUpdater {
    type In = ();
    type Internal = ();
    type Out = usize;

    fn init(ef: &'static EventFactory) -> Pin<Box<dyn Future<Output = InitResult<Self>>>> {
        async move {
            Self::initialize_internal(ef).await
        }.boxed()
    }
    fn update(&self, event: ScheduleEvent<(), ()>, ef: &'static EventFactory) -> Pin<Box<dyn Future<Output = UpdateResult<Self>>>> {
        let new_updater = self.clone();
        async move {
            new_updater.update_internal(event, ef).await
        }.boxed()
    }
}