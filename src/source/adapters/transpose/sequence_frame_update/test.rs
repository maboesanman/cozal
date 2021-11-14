use async_trait::async_trait;
use matches::assert_matches;
use rand::Rng;

use super::SequenceFrameUpdate;
use crate::source::adapters::transpose::input_buffer::InputBuffer;
use crate::source::adapters::transpose::sequence_frame_update::{
    SequenceFrameUpdateInner,
    SequenceFrameUpdatePoll,
};
use crate::test::test_waker::DummyWaker;
use crate::transposer::context::{HandleScheduleContext, InitContext, InterpolateContext};
use crate::transposer::Transposer;

#[derive(Clone, Debug)]
struct TestTransposer {
    counter: usize,
}

#[async_trait(?Send)]
impl Transposer for TestTransposer {
    type Time = usize;

    type InputState = ();

    type OutputState = ();

    type Input = ();

    type Scheduled = ();

    type Output = usize;

    async fn init(&mut self, cx: &mut dyn InitContext<Self>) {
        self.counter = 0;
        cx.schedule_event(0, ()).unwrap();
    }

    async fn handle_scheduled(
        &mut self,
        time: Self::Time,
        _payload: Self::Scheduled,
        cx: &mut dyn HandleScheduleContext<Self>,
    ) {
        cx.schedule_event(time + 1, ()).unwrap();

        self.counter += 1;
        cx.emit_event(self.counter * 10);
    }

    async fn interpolate(
        &self,
        _base_time: Self::Time,
        _interpolated_time: Self::Time,
        _state: Self::InputState,
        _cx: &mut dyn InterpolateContext<Self>,
    ) -> Self::OutputState {
        unimplemented!()
    }
}

#[test]
fn init_saturate() {
    let transposer = TestTransposer {
        counter: 17
    };
    let rng_seed = rand::thread_rng().gen();
    let mut input_buffer = InputBuffer::new();

    let mut init = SequenceFrameUpdate::new_init(transposer, rng_seed);

    let (waker, _) = DummyWaker::new();

    assert_matches!(
        init.poll(waker),
        Ok(SequenceFrameUpdatePoll::ReadyNoOutputs)
    );

    let mut scheduled = init.next_unsaturated(&mut input_buffer).unwrap().unwrap();
    scheduled.saturate_clone(&mut init).unwrap();

    let (waker, _) = DummyWaker::new();

    assert_matches!(
        scheduled.poll(waker),
        Ok(SequenceFrameUpdatePoll::ReadyOutputs(_))
    );

    for i in 2..100 {
        let mut scheduled_next = scheduled
            .next_unsaturated(&mut input_buffer)
            .unwrap()
            .unwrap();
        scheduled_next.saturate_take(&mut scheduled).unwrap();

        let (waker, _) = DummyWaker::new();

        let poll_result = scheduled_next.poll(waker);
        if let SequenceFrameUpdatePoll::ReadyOutputs(o) = poll_result.unwrap() {
            assert_eq!(o.len(), 1);
            assert_eq!(*o.first().unwrap(), i * 10);
        }

        scheduled = scheduled_next;
    }
}
