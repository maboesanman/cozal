use async_trait::async_trait;
use futures_test::future::FutureTestExt;
use rand::Rng;

use super::evaluate_to;
use crate::context::{HandleInputContext, HandleScheduleContext, InitContext, InterpolateContext};
use crate::Transposer;

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
        async {
            self.counter = 0;
            cx.schedule_event(1, ()).unwrap();
            // let _s = cx.get_input_state().await;
        }
        .pending_once()
        .await
    }

    async fn handle_input(
        &mut self,
        _time: Self::Time,
        inputs: &[Self::Input],
        cx: &mut dyn HandleInputContext<'_, Self>,
    ) {
        async {
            self.counter += inputs.len();
            cx.emit_event(self.counter * 10).await;
            // let _s = cx.get_input_state().await;
        }
        .pending_once()
        .await
    }

    async fn handle_scheduled(
        &mut self,
        time: Self::Time,
        _payload: Self::Scheduled,
        cx: &mut dyn HandleScheduleContext<'_, Self>,
    ) {
        async {
            cx.schedule_event(time + 1, ()).unwrap();

            self.counter += 1;
            cx.emit_event(self.counter * 10).await;
            // let _s = cx.get_input_state().await;
        }
        .pending_once()
        .await
    }

    async fn interpolate(
        &self,
        _base_time: Self::Time,
        _interpolated_time: Self::Time,
        cx: &mut dyn InterpolateContext<'_, Self>,
    ) -> Self::OutputState {
        let _s = cx.get_input_state().await;
        async { self.counter }.pending_once().await
    }
}

#[test]
fn basic() {
    let transposer = TestTransposer {
        counter: 17
    };
    let rng_seed = rand::thread_rng().gen();

    // let state_fn = |_| async { core::future::ready(()).pending_once().await };
    let state_fn = |_| async { core::future::ready(()).await };

    let fut = evaluate_to(
        transposer,
        100,
        vec![(10, ()), (15, ()), (27, ()), (200, ())],
        state_fn,
        rng_seed,
    );

    let (x, value) = futures_executor::block_on(fut);

    // 100 from scheduled events, 3 from input events
    assert_eq!(x.len(), 103);
    assert_eq!(value, 100 + 3);
}
