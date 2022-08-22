use core::pin::Pin;

use async_trait::async_trait;
use matches::assert_matches;
use rand::Rng;
use util::dummy_waker::DummyWaker;

use super::StepPoll;
use crate::context::{HandleInputContext, HandleScheduleContext, InitContext, InterpolateContext};
use crate::step::Step;
use crate::Transposer;

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

    type Output = ();

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
        cx.emit_event(()).await;
    }

    async fn handle_input(
        &mut self,
        _time: Self::Time,
        _inputs: &[Self::Input],
        cx: &mut dyn HandleInputContext<'_, Self>,
    ) {
        self.counter += 1;
        cx.emit_event(()).await;
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
fn next_unsaturated_same_time() {
    let transposer = TestTransposer {
        counter: 17
    };
    let rng_seed = rand::thread_rng().gen();
    let mut next_input = Some((1, vec![()].into_boxed_slice()));

    let mut step = Step::<_>::new_init(transposer, rng_seed);

    Pin::new(&mut step).poll(DummyWaker::dummy()).unwrap();

    {
        let mut next = step.next_unsaturated(&mut next_input).unwrap().unwrap();
        next.saturate_take(&mut step).unwrap();

        let mut pin = Pin::new(&mut next);
        assert_matches!(pin.poll(DummyWaker::dummy()), Ok(StepPoll::Emitted(())));
        assert_matches!(pin.poll(DummyWaker::dummy()), Ok(StepPoll::Ready));

        step = next;
    }
    {
        let mut next = step.next_unsaturated(&mut next_input).unwrap().unwrap();
        next.saturate_take(&mut step).unwrap();

        let mut pin = Pin::new(&mut next);
        assert_matches!(pin.poll(DummyWaker::dummy()), Ok(StepPoll::Emitted(())));
        assert_matches!(pin.poll(DummyWaker::dummy()), Ok(StepPoll::Emitted(())));
        assert_matches!(pin.poll(DummyWaker::dummy()), Ok(StepPoll::Ready));

        step = next;
    }
    {
        let mut next = step.next_unsaturated(&mut next_input).unwrap().unwrap();
        next.saturate_take(&mut step).unwrap();

        let mut pin = Pin::new(&mut next);
        assert_matches!(pin.poll(DummyWaker::dummy()), Ok(StepPoll::Emitted(())));
        assert_matches!(pin.poll(DummyWaker::dummy()), Ok(StepPoll::Ready));

        step = next;
    }

    let _step = step;
}

#[test]
fn do_some_clones() {
    let transposer = TestTransposer {
        counter: 17
    };
    let rng_seed = rand::thread_rng().gen();
    let mut next_input = Some((1, vec![()].into_boxed_slice()));

    let mut step = Step::<_>::new_init(transposer, rng_seed);

    Pin::new(&mut step).poll(DummyWaker::dummy()).unwrap();

    {
        let mut next = step.next_unsaturated(&mut next_input).unwrap().unwrap();
        next.saturate_clone(&step).unwrap();

        let mut pin = Pin::new(&mut next);
        assert_matches!(pin.poll(DummyWaker::dummy()), Ok(StepPoll::Emitted(())));
        assert_matches!(pin.poll(DummyWaker::dummy()), Ok(StepPoll::Ready));

        step = next;
    }
    {
        let mut next = step.next_unsaturated(&mut next_input).unwrap().unwrap();
        next.saturate_clone(&step).unwrap();

        let mut pin = Pin::new(&mut next);
        assert_matches!(pin.poll(DummyWaker::dummy()), Ok(StepPoll::Emitted(())));
        assert_matches!(pin.poll(DummyWaker::dummy()), Ok(StepPoll::Emitted(())));
        assert_matches!(pin.poll(DummyWaker::dummy()), Ok(StepPoll::Ready));

        step = next;
    }
    {
        let mut next = step.next_unsaturated(&mut next_input).unwrap().unwrap();
        next.saturate_clone(&step).unwrap();

        let mut pin = Pin::new(&mut next);
        assert_matches!(pin.poll(DummyWaker::dummy()), Ok(StepPoll::Emitted(())));
        assert_matches!(pin.poll(DummyWaker::dummy()), Ok(StepPoll::Ready));

        step = next;
    }

    let _step = step;
}