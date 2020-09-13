use crate::core::schedule_stream::ScheduleStreamExt;
use crate::core::Transposer;
use crate::core::{
    event::RollbackPayload,
    transposer::{InitResult, TransposerContext, TransposerEngine, UpdateResult},
    Event,
};
use async_trait::async_trait;
use futures::Stream;
use std::task::Poll;

struct EmptyStream {}

impl Stream for EmptyStream {
    type Item = Event<usize, RollbackPayload<!>>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        Poll::Pending
    }
}

#[derive(Clone)]
struct TestTransposer {
    event_calls: Vec<usize>,
}

impl TestTransposer {
    pub fn new() -> Self {
        Self {
            event_calls: Vec::new(),
        }
    }
}

#[async_trait]
impl Transposer for TestTransposer {
    type Time = usize;

    type Input = !;

    type Scheduled = usize;

    type Output = Vec<usize>;

    async fn init_events(&mut self, _cx: &TransposerContext) -> InitResult<Self> {
        InitResult {
            new_events: vec![
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
            ],
            emitted_events: Vec::new(),
        }
    }

    async fn handle_input(
        &mut self,
        time: Self::Time,
        inputs: &[Self::Input],
        cx: &TransposerContext,
    ) -> UpdateResult<Self> {
        UpdateResult::default()
    }

    async fn handle_scheduled(
        &mut self,
        time: Self::Time,
        payload: &Self::Scheduled,
        cx: &TransposerContext,
    ) -> UpdateResult<Self> {
        self.event_calls.push(*payload);
        let mut new_events = Vec::new();
        if payload % 2 == 1 {
            new_events.push(Event {
                timestamp: time * 2,
                payload: payload * 2,
            })
        };
        let emitted_events = vec![self.event_calls.clone()];
        UpdateResult {
            expired_events: Vec::new(),
            new_events,
            emitted_events,
            exit: false,
        }
    }
}
#[test]
fn test_events_scheduled_correctly() {
    let transposer = TestTransposer::new();
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
        assert_eq!(payload, vec![1, 2, 3, 4, 6, 8]);
    } else {
        panic!()
    }
}
