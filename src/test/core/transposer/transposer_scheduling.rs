use crate::core::schedule_stream::ScheduleStreamExt;
use crate::core::Transposer;
use crate::core::{
    event::RollbackPayload,
    transposer::{InitResult, TransposerContext, TransposerEngine, TransposerEvent, UpdateResult},
    Event,
};
use async_trait::async_trait;
use futures::Stream;
use std::task::Poll;

struct EmptyStream {}

impl Stream for EmptyStream {
    type Item = Event<usize, RollbackPayload<()>>;

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

#[async_trait]
impl Transposer for TestTransposer {
    type Time = usize;

    type External = ();

    type Internal = usize;

    type Out = Vec<usize>;

    async fn init(_cx: &TransposerContext) -> InitResult<Self> {
        InitResult {
            transposer: Self {
                event_calls: Vec::new(),
            },
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

    async fn update(
        &self,
        _cx: &TransposerContext,
        events: Vec<&TransposerEvent<Self>>,
    ) -> UpdateResult<Self> {
        let mut new_transposer = self.clone();
        let mut new_events = Vec::new();
        for event in events {
            match event {
                TransposerEvent::Internal(e) => {
                    new_transposer.event_calls.push(e.event.payload);
                    if e.event.payload % 2 == 1 {
                        new_events.push(Event {
                            timestamp: e.event.timestamp * 2,
                            payload: e.event.timestamp * 2,
                        })
                    };
                }
                _ => {}
            };
        }
        let emitted_events = vec![new_transposer.event_calls.clone()];
        let new_transposer = Some(new_transposer);
        UpdateResult {
            expired_events: Vec::new(),
            new_events,
            emitted_events,
            exit: false,
            new_transposer,
        }
    }
}
#[test]
fn test_init_events_scheduled_correctly() {
    let engine = futures::executor::block_on(TransposerEngine::<'_, TestTransposer, _>::new(
        EmptyStream {},
    ));
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
