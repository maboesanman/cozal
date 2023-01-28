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
        time: Self::Time,
        _payload: Self::Scheduled,
        cx: &mut dyn HandleScheduleContext<'_, Self>,
    ) {
        cx.schedule_event(time + 1, ()).unwrap();

        self.counter += 1;
        cx.emit_event(()).await;
    }

    async fn interpolate(
        &self,
        _base_time: Self::Time,
        _interpolated_time: Self::Time,
        _cx: &mut dyn InterpolateContext<'_, Self>,
    ) -> Self::OutputState {
        self.counter
    }
}

#[test]
fn next_scheduled_unsaturated() {
    let transposer = TestTransposer {
        counter: 17
    };
    let rng_seed = rand::thread_rng().gen();

    let mut step = Step::<_, NoInput>::new_init(transposer, rng_seed);

    Pin::new(&mut step).poll(DummyWaker::dummy()).unwrap();

    for i in 1..100 {
        let mut next = step.next_scheduled_unsaturated().unwrap().unwrap();
        next.saturate_take(&mut step).unwrap();

        assert_matches!(next.poll(DummyWaker::dummy()), Ok(StepPoll::Emitted(())));
        assert_matches!(next.poll(DummyWaker::dummy()), Ok(StepPoll::Ready));

        let interpolated = futures_executor::block_on(next.interpolate(i + 1).unwrap());
        assert_eq!(interpolated, i);

        step = next;
    }
}
