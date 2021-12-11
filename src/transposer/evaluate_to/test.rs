use std::pin::Pin;
use std::task::{Context, Poll};

use async_trait::async_trait;
use futures_core::Future;
use matches::assert_matches;
use rand::Rng;

use super::evaluate_to;
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

    type OutputState = usize;

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
        cx.emit_event(self.counter * 10);
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
fn basic() {
    let transposer = TestTransposer {
        counter: 17
    };
    let rng_seed = rand::thread_rng().gen();

    let fut = evaluate_to(
        transposer,
        100,
        Vec::new(),
        |_| core::future::ready(()),
        rng_seed,
    );

    let (_, value) = futures_executor::block_on(fut);

    assert_eq!(value, 101)
}
