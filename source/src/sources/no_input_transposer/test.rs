use std::time::Instant;

use transposer::context::{HandleScheduleContext, InitContext, InterpolateContext};
use transposer::step::NoInputManager;
use transposer::Transposer;

#[derive(Clone)]
pub(crate) struct TestTransposer {
    value: usize,
}

#[allow(dead_code)]
impl TestTransposer {
    pub fn new(init_events: Vec<(usize, usize)>) -> Self {
        Self {
            init_events,

            handle_record: im::Vector::new(),
        }
    }
}

impl Transposer for TestTransposer {
    type Time = Instant;

    type OutputState = Vec<(HandleRecord, u64)>;

    type Scheduled = usize;

    type OutputEvent = usize;

    // set up with macro
    type InputStateManager = NoInputManager;

    async fn init(&mut self, cx: &mut dyn InitContext<'_, Self>) {
        todo!()
    }

    async fn handle_scheduled(
        &mut self,
        time: Self::Time,
        payload: Self::Scheduled,
        cx: &mut dyn HandleScheduleContext<'_, Self>,
    ) {
        todo!()
    }

    async fn interpolate(
        &self,
        _base_time: Self::Time,
        _interpolated_time: Self::Time,
        _cx: &mut dyn InterpolateContext<'_, Self>,
    ) -> Self::OutputState {
        todo!()
    }
}
