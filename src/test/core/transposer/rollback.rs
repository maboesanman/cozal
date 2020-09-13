use crate::core::schedule_stream::ScheduleStreamExt;
use crate::core::{
    event::RollbackPayload,
    transposer::TransposerEngine,
    Event,
};

use super::test_transposer::{TestTransposer, EventCall};

#[test]
fn basic_rollback() {
    let transposer = TestTransposer::new(Vec::new());
    let (sender, receiver) = flume::unbounded();
    let engine = futures::executor::block_on(TransposerEngine::new(transposer, receiver));
    let stream = engine.to_target(5);
    let mut iter = futures::executor::block_on_stream(stream);

    assert!(sender.send(Event {
        timestamp: 1,
        payload: RollbackPayload::Payload(1),
    }).is_ok());

    assert!(sender.send(Event {
        timestamp: 3,
        payload: RollbackPayload::Payload(3),
    }).is_ok());

    iter.next();
    iter.next();

    assert!(sender.send(Event {
        timestamp: 2,
        payload: RollbackPayload::Payload(2),
    }).is_ok());

    if let Some(Event {
        payload: RollbackPayload::Payload(payload),
        ..
    }) = iter.next()
    {
        assert_eq!(payload, vec![
            EventCall::Input(1),
            EventCall::Input(2),
            EventCall::Input(3),
        ]);
    } else {
        panic!()
    }
}
