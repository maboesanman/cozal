use core::pin::Pin;

use matches::assert_matches;
use rand::Rng;
use util::dummy_waker::DummyWaker;

use super::{NoInput, NoInputManager, StepPoll};
use crate::context::{HandleScheduleContext, InitContext, InterpolateContext};
use crate::step::Step;
use crate::Transposer;

#[derive(Clone, Debug)]
struct TestTransposer {
    counter: u32,
}

impl Transposer for TestTransposer {
    type Time = u32;

    type OutputState = u32;

    type Scheduled = ();

    type OutputEvent = ();

    type InputStateManager = NoInputManager;

    async fn init(&mut self, cx: &mut dyn InitContext<'_, Self>) {
        self.counter = 0;
        cx.schedule_event(1, ()).unwrap();
    }

    async fn handle_scheduled(
        &mut self,
        _payload: Self::Scheduled,
        cx: &mut dyn HandleScheduleContext<'_, Self>,
    ) {
        cx.schedule_event(cx.current_time() + 1, ()).unwrap();

        self.counter += 1;
        cx.emit_event(()).await;
    }

    async fn interpolate(&self, _cx: &mut dyn InterpolateContext<'_, Self>) -> Self::OutputState {
        self.counter
    }
}

#[test]
fn next_scheduled_unsaturated_take() {
    let transposer = TestTransposer {
        counter: 17
    };
    let rng_seed = rand::thread_rng().gen();

    let mut step = Step::<_, NoInput>::new_init(transposer, 0, rng_seed);

    let waker = DummyWaker::dummy();
    Pin::new(&mut step).poll(&waker).unwrap();

    for i in 1..100 {
        let mut next = step.next_scheduled_unsaturated().unwrap().unwrap();
        next.saturate_take(&mut step).unwrap();

        assert_matches!(next.poll(&waker), Ok(StepPoll::Emitted(())));
        assert_matches!(next.poll(&waker), Ok(StepPoll::Ready));

        let interpolated = futures_executor::block_on(next.interpolate(i + 1).unwrap());
        assert_eq!(interpolated, i);

        step = next;
    }
}

#[test]
fn next_scheduled_unsaturated_clone() {
    let transposer = TestTransposer {
        counter: 17
    };
    let rng_seed = rand::thread_rng().gen();

    let mut step = Step::<_, NoInput>::new_init(transposer, 0, rng_seed);

    let waker = DummyWaker::dummy();
    Pin::new(&mut step).poll(&waker).unwrap();

    for i in 1..100 {
        let mut next = step.next_scheduled_unsaturated().unwrap().unwrap();
        next.saturate_clone(&step).unwrap();

        assert_matches!(next.poll(&waker), Ok(StepPoll::Emitted(())));
        assert_matches!(next.poll(&waker), Ok(StepPoll::Ready));

        let interpolated = futures_executor::block_on(next.interpolate(i + 1).unwrap());
        assert_eq!(interpolated, i);

        step = next;
    }
}

#[test]
fn next_scheduled_unsaturated_desaturate() {
    let transposer = TestTransposer {
        counter: 17
    };
    let rng_seed = rand::thread_rng().gen();

    let mut init = Step::<_, NoInput>::new_init(transposer, 0, rng_seed);

    let waker = DummyWaker::dummy();
    Pin::new(&mut init).poll(&waker).unwrap();

    let mut step1 = init.next_scheduled_unsaturated().unwrap().unwrap();
    step1.saturate_clone(&init).unwrap();

    // emits the event the first time
    assert_matches!(step1.poll(&waker), Ok(StepPoll::Emitted(())));
    assert_matches!(step1.poll(&waker), Ok(StepPoll::Ready));

    step1.desaturate();
    step1.saturate_clone(&init).unwrap();

    // doesn't re-emit the event
    assert_matches!(step1.poll(&waker), Ok(StepPoll::Ready));

    step1.desaturate();
    step1.saturate_clone(&init).unwrap();

    // doesn't re-emit the event
    assert_matches!(step1.poll(&waker), Ok(StepPoll::Ready));

    step1.desaturate();
}
