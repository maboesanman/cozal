use core::pin::Pin;
use core::task::Context;
use std::sync::Arc;

use async_trait::async_trait;
use matches::assert_matches;
use rand::Rng;
use util::dummy_waker::DummyWaker;

use crate::context::{HandleScheduleContext, InitContext, InterpolateContext};
use crate::schedule_storage::DefaultStorage;
use crate::step::lazy_state::LazyState;
use crate::step::sub_step::SubStep;
use crate::step::StepPoll;
use crate::Transposer;

#[derive(Clone, Debug, PartialEq, Eq)]
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

    async fn init(&mut self, cx: &mut dyn InitContext<'_, Self>) {
        self.counter = 0;
        cx.schedule_event(0, ()).unwrap();
    }

    async fn handle_scheduled(
        &mut self,
        time: Self::Time,
        _payload: Self::Scheduled,
        cx: &mut dyn HandleScheduleContext<'_, Self>,
    ) {
        cx.schedule_event(time + 1, ()).unwrap();

        self.counter += 1;
        cx.emit_event(self.counter * 10).await;
    }

    async fn interpolate(
        &self,
        _base_time: Self::Time,
        _interpolated_time: Self::Time,
        _cx: &mut dyn InterpolateContext<'_, Self>,
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

    let s = Arc::new(LazyState::new());
    let mut init = SubStep::<_, DefaultStorage>::new_init(transposer, rng_seed, &s);

    let waker = DummyWaker::dummy();
    let mut cx = Context::from_waker(&waker);

    assert_matches!(Pin::new(&mut init).poll(&mut cx), Ok(StepPoll::Ready));

    let s = Arc::new(LazyState::new());
    let mut scheduled = init;

    for i in 1..100 {
        let mut scheduled_next = scheduled
            .next_unsaturated(&mut next_input, &s)
            .unwrap()
            .unwrap();
        scheduled_next.saturate_take(&mut scheduled).unwrap();

        let poll_result = Pin::new(&mut scheduled_next).poll(&mut cx);
        assert_eq!(poll_result, Ok(StepPoll::Emitted(i * 10)));

        let poll_result = Pin::new(&mut scheduled_next).poll(&mut cx);
        assert_eq!(poll_result, Ok(StepPoll::Ready));

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

    let s = Arc::new(LazyState::new());
    let mut init = SubStep::<_, DefaultStorage>::new_init(transposer, rng_seed, &s);

    let waker = DummyWaker::dummy();
    let mut cx = Context::from_waker(&waker);

    assert_matches!(Pin::new(&mut init).poll(&mut cx), Ok(StepPoll::Ready));

    let s = Arc::new(LazyState::new());
    let mut scheduled = init;

    for i in 1..100 {
        let mut scheduled_next = scheduled
            .next_unsaturated(&mut next_input, &s)
            .unwrap()
            .unwrap();
        scheduled_next.saturate_clone(&mut scheduled).unwrap();

        let poll_result = Pin::new(&mut scheduled_next).poll(&mut cx);
        assert_eq!(poll_result, Ok(StepPoll::Emitted(i * 10)));

        let poll_result = Pin::new(&mut scheduled_next).poll(&mut cx);
        assert_eq!(poll_result, Ok(StepPoll::Ready));

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

    let s = Arc::new(LazyState::new());
    let mut init = SubStep::<_, DefaultStorage>::new_init(transposer, rng_seed, &s);
    let waker = DummyWaker::dummy();
    let mut cx = Context::from_waker(&waker);

    assert_matches!(Pin::new(&mut init).poll(&mut cx), Ok(StepPoll::Ready));

    let s = Arc::new(LazyState::new());
    let mut scheduled1 = init.next_unsaturated(&mut next_input, &s).unwrap().unwrap();
    scheduled1.saturate_clone(&mut init).unwrap();
    assert_matches!(
        Pin::new(&mut scheduled1).poll(&mut cx),
        Ok(StepPoll::Emitted(10))
    );
    assert_matches!(Pin::new(&mut scheduled1).poll(&mut cx), Ok(StepPoll::Ready));

    let s = Arc::new(LazyState::new());
    let mut scheduled2 = scheduled1
        .next_unsaturated(&mut next_input, &s)
        .unwrap()
        .unwrap();
    scheduled2.saturate_clone(&mut scheduled1).unwrap();
    assert_matches!(
        Pin::new(&mut scheduled2).poll(&mut cx),
        Ok(StepPoll::Emitted(20))
    );
    assert_matches!(Pin::new(&mut scheduled2).poll(&mut cx), Ok(StepPoll::Ready));

    scheduled1.desaturate().unwrap();
    scheduled1.saturate_clone(&mut init).unwrap();
    // second time they don't emit
    assert_matches!(Pin::new(&mut scheduled1).poll(&mut cx), Ok(StepPoll::Ready));

    scheduled2.desaturate().unwrap();
    scheduled2.saturate_clone(&mut scheduled1).unwrap();
    // second time they don't emit
    assert_matches!(Pin::new(&mut scheduled2).poll(&mut cx), Ok(StepPoll::Ready));

    init.desaturate().unwrap();
}

#[test]
fn next_unsaturated_same_time() {
    let transposer = TestTransposer {
        counter: 17
    };
    let rng_seed = rand::thread_rng().gen();
    let mut next_input = Some((1, vec![()].into_boxed_slice()));

    let s = Arc::new(LazyState::new());
    let mut step = SubStep::<_, DefaultStorage>::new_init(transposer, rng_seed, &s);

    let waker = DummyWaker::dummy();
    let mut cx = Context::from_waker(&waker);

    assert_matches!(Pin::new(&mut step).poll(&mut cx), Ok(StepPoll::Ready));

    let s = Arc::new(LazyState::new());

    // let next = step.next_unsaturated(&mut next_input, &s).unwrap();

    {
        let mut next = step.next_unsaturated(&mut next_input, &s).unwrap().unwrap();
        next.saturate_take(&mut step).unwrap();

        assert_matches!(Pin::new(&mut next).poll(&mut cx), Ok(StepPoll::Emitted(10)));
        assert_matches!(Pin::new(&mut next).poll(&mut cx), Ok(StepPoll::Ready));

        step = next;
    }
    {
        let mut next = step.next_unsaturated(&mut next_input, &s).unwrap().unwrap();
        next.saturate_take(&mut step).unwrap();

        assert_matches!(Pin::new(&mut next).poll(&mut cx), Ok(StepPoll::Ready));

        step = next;
    }
    {
        let mut next = step.next_unsaturated_same_time().unwrap().unwrap();
        next.saturate_take(&mut step).unwrap();

        assert_matches!(Pin::new(&mut next).poll(&mut cx), Ok(StepPoll::Emitted(20)));
        assert_matches!(Pin::new(&mut next).poll(&mut cx), Ok(StepPoll::Ready));

        step = next;
    }
    {
        let mut next = step.next_unsaturated(&mut next_input, &s).unwrap().unwrap();
        next.saturate_take(&mut step).unwrap();

        assert_matches!(Pin::new(&mut next).poll(&mut cx), Ok(StepPoll::Emitted(30)));
        assert_matches!(Pin::new(&mut next).poll(&mut cx), Ok(StepPoll::Ready));

        step = next;
    }

    let _step = step;
}
