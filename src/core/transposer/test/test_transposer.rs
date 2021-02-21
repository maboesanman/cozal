use crate::core::transposer::context::{InitContext, InputContext, ScheduleContext, InterpolateContext};
use crate::core::Transposer;
use async_trait::async_trait;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HandleRecord {
    Input(usize, Vec<usize>),
    Scheduled(usize, usize),
}

#[derive(Clone)]
pub(crate) struct TestTransposer {
    init_events: Vec<(usize, usize)>,

    pub handle_record: im::Vector<HandleRecord>,
}

impl TestTransposer {
    pub fn new(init_events: Vec<(usize, usize)>) -> Self {
        Self {
            init_events,

            handle_record: im::Vector::new(),
        }
    }
}

#[async_trait(?Send)]
impl Transposer for TestTransposer {
    type Time = usize;

    type InputState = usize;

    type OutputState = Self;

    type Input = usize;

    type Scheduled = usize;

    type Output = (usize, HandleRecord);

    async fn init(&mut self, cx: &mut dyn InitContext<'_, Self>) {
        for (time, payload) in self.init_events.drain(..) {
            let _ = cx.schedule_event(time, payload);
        }
    }

    async fn handle_input(
        &mut self,
        time: Self::Time,
        inputs: &[Self::Input],
        cx: &mut dyn InputContext<'_, Self>,
    ) {
        let mut vec = Vec::new();
        for payload in inputs {
            vec.push(*payload);
        }
        let record = HandleRecord::Input(time, vec);
        self.handle_record.push_back(record.clone());

        let state = *cx.get_input_state().await.unwrap();
        cx.emit_event((state, record));
    }

    async fn handle_scheduled(
        &mut self,
        time: Self::Time,
        payload: Self::Scheduled,
        cx: &mut dyn ScheduleContext<'_, Self>,
    ) {
        let record = HandleRecord::Scheduled(time, payload);
        self.handle_record.push_back(record.clone());

        let state = *cx.get_input_state().await.unwrap();
        cx.emit_event((state, record));
    }

    async fn interpolate(
        &self,
        _base_time: Self::Time,
        _interpolated_time: Self::Time,
        _cx: &mut dyn InterpolateContext<'_, Self>
    ) -> Self::OutputState {
        self.clone()
    }
}
