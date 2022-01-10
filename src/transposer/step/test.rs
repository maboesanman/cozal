use std::pin::Pin;

use async_trait::async_trait;
use rand::Rng;

use super::{StepPoll, StepPollResult};
use crate::transposer::context::{
    HandleInputContext,
    HandleScheduleContext,
    InitContext,
    InterpolateContext,
};
use crate::transposer::step::Step;
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
        cx.emit_event(());
    }

    async fn handle_input(
        &mut self,
        _time: Self::Time,
        _inputs: &[Self::Input],
        cx: &mut dyn HandleInputContext<'_, Self>,
    ) {
        self.counter += 1;
        cx.emit_event(());
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

    Pin::new(&mut step)
        .poll_progress(DummyWaker::dummy())
        .unwrap();

    {
        let mut next = step.next_unsaturated(&mut next_input).unwrap().unwrap();
        next.saturate_take(&mut step).unwrap();

        let poll_result = Pin::new(&mut next).poll_progress(DummyWaker::dummy());
        if let Ok(StepPoll {
            result: StepPollResult::Ready,
            outputs,
        }) = poll_result
        {
            assert_eq!(outputs.len(), 1);
        }

        step = next;
    }
    {
        let mut next = step.next_unsaturated(&mut next_input).unwrap().unwrap();
        next.saturate_take(&mut step).unwrap();

        let poll_result = Pin::new(&mut next).poll_progress(DummyWaker::dummy());
        if let Ok(StepPoll {
            result: StepPollResult::Ready,
            outputs,
        }) = poll_result
        {
            assert_eq!(outputs.len(), 2);
        }

        step = next;
    }
    {
        let mut next = step.next_unsaturated(&mut next_input).unwrap().unwrap();
        next.saturate_take(&mut step).unwrap();

        let poll_result = Pin::new(&mut next).poll_progress(DummyWaker::dummy());
        if let Ok(StepPoll {
            result: StepPollResult::Ready,
            outputs,
        }) = poll_result
        {
            assert_eq!(outputs.len(), 1);
        }

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

    Pin::new(&mut step)
        .poll_progress(DummyWaker::dummy())
        .unwrap();

    {
        let mut next = step.next_unsaturated(&mut next_input).unwrap().unwrap();
        next.saturate_clone(&step).unwrap();

        let poll_result = Pin::new(&mut next).poll_progress(DummyWaker::dummy());
        if let Ok(StepPoll {
            result: StepPollResult::Ready,
            outputs,
        }) = poll_result
        {
            assert_eq!(outputs.len(), 1);
        }

        step = next;
    }
    {
        let mut next = step.next_unsaturated(&mut next_input).unwrap().unwrap();
        next.saturate_clone(&step).unwrap();

        let poll_result = Pin::new(&mut next).poll_progress(DummyWaker::dummy());
        if let Ok(StepPoll {
            result: StepPollResult::Ready,
            outputs,
        }) = poll_result
        {
            assert_eq!(outputs.len(), 2);
        }

        step = next;
    }
    {
        let mut next = step.next_unsaturated(&mut next_input).unwrap().unwrap();
        next.saturate_clone(&step).unwrap();

        let poll_result = Pin::new(&mut next).poll_progress(DummyWaker::dummy());
        if let Ok(StepPoll {
            result: StepPollResult::Ready,
            outputs,
        }) = poll_result
        {
            assert_eq!(outputs.len(), 1);
        }

        step = next;
    }

    let _step = step;
}
