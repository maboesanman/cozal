use crate::core::event::{EventContent, EventTimestamp, ScheduleEvent};
use crate::core::updater::{InitResult, UpdateResult, Updater};
use futures::{Future, FutureExt};
use std::pin::Pin;
use std::time::Duration;

#[derive(Clone)]
pub struct MyUpdater {
    count: usize,
}

impl MyUpdater {
    async fn initialize_internal() -> InitResult<Self> {
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
    async fn update_internal(mut self, event: ScheduleEvent<(), ()>) -> UpdateResult<Self> {
        match event {
            ScheduleEvent::External(_) => {
                self.count -= 1;
                let new_out_event = EventContent {
                    timestamp: EventTimestamp {
                        time: event.timestamp().time + Duration::from_secs(1),
                        priority: 1,
                    },
                    payload: self.count,
                };
                UpdateResult {
                    new_updater: self,
                    trigger: event,
                    expired_events: vec![],
                    new_events: vec![],
                    emitted_events: vec![new_out_event],
                }
            }
            ScheduleEvent::Internal(_) => {
                self.count += 1;
                let new_in_event = EventContent {
                    timestamp: EventTimestamp {
                        time: event.timestamp().time + Duration::from_secs(1),
                        priority: 1,
                    },
                    payload: (),
                };
                let new_out_event = EventContent {
                    timestamp: EventTimestamp {
                        time: event.timestamp().time + Duration::from_secs(1),
                        priority: 1,
                    },
                    payload: self.count,
                };
                UpdateResult {
                    new_updater: self,
                    trigger: event,
                    expired_events: vec![],
                    new_events: vec![new_in_event],
                    emitted_events: vec![new_out_event],
                }
            }
        }
    }
}

impl Updater for MyUpdater {
    type In = ();
    type Internal = ();
    type Out = usize;

    fn init() -> Pin<Box<dyn Future<Output = InitResult<Self>>>> {
        async move { Self::initialize_internal().await }.boxed()
    }
    fn update(
        &self,
        event: ScheduleEvent<(), ()>,
    ) -> Pin<Box<dyn Future<Output = UpdateResult<Self>>>> {
        let new_updater = self.clone();
        async move { new_updater.update_internal(event).await }.boxed()
    }
}
