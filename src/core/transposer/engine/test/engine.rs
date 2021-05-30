use std::{pin::Pin, task::Context};

use crate::{core::{event_state_stream::{EventStateStream, EventStateStreamExt, iter_event_state_stream::IterEventStateStream}, transposer::test::test_transposer::TestTransposer}, test::test_waker::DummyWaker};

#[test]
fn basic_test() {
    // this event state stream emits an event every 10, with payload i, and state i^2
    let test_input_iter = (0..).map(|i| (10 * i, i, i * i));
    let test_input = IterEventStateStream::new(test_input_iter, 0);

    let mut engine = test_input.into_engine::<_, 20>(TestTransposer::new(vec![]));
    let engine_ref = &mut engine;

    let engine_pin = unsafe { Pin::new_unchecked(engine_ref) };

    let (waker, _) = DummyWaker::new();

    let mut cx = Context::from_waker(&waker);

    match engine_pin.poll(15, &mut cx) {
        crate::core::event_state_stream::EventStatePoll::Pending => {
            println!("Pending")
        }
        crate::core::event_state_stream::EventStatePoll::Rollback(t) => {
            println!("Rollback({})", t);
        }
        crate::core::event_state_stream::EventStatePoll::Event(t, e) => {
            println!("Event({}, _)", t);
        }
        crate::core::event_state_stream::EventStatePoll::Scheduled(t, s) => {
            println!("Scheduled({}, _)", t);
        }
        crate::core::event_state_stream::EventStatePoll::Ready(s) => {
            println!("Ready(_)")
        }
    }
}