use async_trait::async_trait;
use matches::assert_matches;
use rand::Rng;

use super::{SequenceFrame, SequenceFramePoll};
use crate::transposer::context::{HandleScheduleContext, InitContext, InterpolateContext};
use crate::transposer::Transposer;
use crate::util::dummy_waker::DummyWaker;

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
fn saturate_take() {
    let transposer = TestTransposer {
        counter: 17
    };
    let rng_seed = rand::thread_rng().gen();
    let mut next_input = None;

    let mut init = SequenceFrame::new_init(transposer, rng_seed);

    let (waker, _) = DummyWaker::new();

    assert_matches!(init.poll(waker), Ok(SequenceFramePoll::ReadyNoOutputs));

    let mut scheduled = init;

    for i in 1..100 {
        let mut scheduled_next = scheduled
            .next_unsaturated(&mut next_input)
            .unwrap()
            .unwrap();
        scheduled_next.saturate_take(&mut scheduled).unwrap();

        let (waker, _) = DummyWaker::new();

        let poll_result = scheduled_next.poll(waker);
        if let SequenceFramePoll::ReadyOutputs(o) = poll_result.unwrap() {
            assert_eq!(o.len(), 1);
            assert_eq!(*o.first().unwrap(), i * 10);
        }

        scheduled = scheduled_next;
    }
}

#[test]
fn saturate_clone() {
    let transposer = TestTransposer {
        counter: 17
    };
    let rng_seed = rand::thread_rng().gen();
    let mut next_input = None;

    let mut init = SequenceFrame::new_init(transposer, rng_seed);

    let (waker, _) = DummyWaker::new();

    assert_matches!(init.poll(waker), Ok(SequenceFramePoll::ReadyNoOutputs));

    let mut scheduled = init;

    for i in 1..100 {
        let mut scheduled_next = scheduled
            .next_unsaturated(&mut next_input)
            .unwrap()
            .unwrap();
        scheduled_next.saturate_clone(&mut scheduled).unwrap();

        let (waker, _) = DummyWaker::new();

        let poll_result = scheduled_next.poll(waker);
        if let SequenceFramePoll::ReadyOutputs(o) = poll_result.unwrap() {
            assert_eq!(o.len(), 1);
            assert_eq!(*o.first().unwrap(), i * 10);
        }

        scheduled = scheduled_next;
    }
}

#[test]
fn desaturate() {
    let transposer = TestTransposer {
        counter: 17
    };
    let rng_seed = rand::thread_rng().gen();
    let mut next_input = None;

    let mut init = SequenceFrame::new_init(transposer, rng_seed);
    init.poll(DummyWaker::new().0).unwrap();

    let mut scheduled1 = init.next_unsaturated(&mut next_input).unwrap().unwrap();
    scheduled1.saturate_clone(&mut init).unwrap();
    scheduled1.poll(DummyWaker::new().0).unwrap();

    let mut scheduled2 = scheduled1
        .next_unsaturated(&mut next_input)
        .unwrap()
        .unwrap();
    scheduled2.saturate_clone(&mut scheduled1).unwrap();
    scheduled2.poll(DummyWaker::new().0).unwrap();

    scheduled1.desaturate().unwrap();
    scheduled1.saturate_clone(&mut init).unwrap();
    scheduled1.poll(DummyWaker::new().0).unwrap();

    scheduled2.desaturate().unwrap();
    scheduled2.saturate_clone(&mut scheduled1).unwrap();
    scheduled2.poll(DummyWaker::new().0).unwrap();

    init.desaturate().unwrap();
}
