use crate::{core::{event_state_stream::{EventStatePoll}, transposer::{test::test_stateful_stream::TestStatefulStream, TransposerEngine}}, test::test_waker::DummyWaker};
use core::{pin::Pin, task::Context};

use crate::core::transposer::test::test_transposer::TestTransposer;

#[test]
fn test_waiting() {
    let transposer = TestTransposer::new(Vec::new());
    let (_sender, input_stream) = TestStatefulStream::<usize, usize, usize>::new(0);
    let engine = TransposerEngine::new(transposer, input_stream);
    let mut engine = futures::executor::block_on(engine);

    let engine_ref = unsafe { Pin::new_unchecked(&mut engine) };
    let (waker, _) = DummyWaker::new();
    let mut cx = Context::from_waker(&waker);

    let poll = engine_ref.poll(2, &mut cx);

    matches!(poll, EventStatePoll::Ready(_));
}
