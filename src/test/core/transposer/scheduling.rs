use crate::core::schedule_stream::ScheduleStreamExt;
use crate::core::{event::RollbackPayload, transposer::TransposerEngine, Event};

use super::test_transposer::{EmptyStream, EventCall, TestTransposer};

#[test]
fn test_events_scheduled_correctly() {
    let transposer = TestTransposer::new(vec![
        Event {
            timestamp: 8,
            payload: 8,
        },
        Event {
            timestamp: 1,
            payload: 1,
        },
        Event {
            timestamp: 4,
            payload: 4,
        },
        Event {
            timestamp: 3,
            payload: 3,
        },
    ]);
    let engine = futures::executor::block_on(TransposerEngine::new(transposer, EmptyStream {}));
    let stream = engine.to_target(100);
    let mut iter = futures::executor::block_on_stream(stream);
    iter.next();
    iter.next();
    iter.next();
    iter.next();
    iter.next();
    if let Some(Event {
        payload: RollbackPayload::Payload(payload),
        ..
    }) = iter.next()
    {
        assert_eq!(
            payload,
            vec![
                EventCall::Scheduled(1),
                EventCall::Scheduled(2),
                EventCall::Scheduled(3),
                EventCall::Scheduled(4),
                EventCall::Scheduled(6),
                EventCall::Scheduled(8),
            ]
        );
    } else {
        panic!()
    }
}
