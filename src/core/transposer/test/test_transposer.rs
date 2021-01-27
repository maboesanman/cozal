use crate::core::transposer::{InitContext, UpdateContext, context::InterpolateContext};
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

#[async_trait]
impl Transposer for TestTransposer {
    type Time = usize;

    type InputState = usize;

    type OutputState = Self;

    type Input = usize;

    type Scheduled = usize;

    type Output = (usize, HandleRecord);

    async fn init<'a>(&'a mut self, cx: &'a mut InitContext<'a, Self>) {
        for (time, payload) in self.init_events.drain(..) {
            let _ = cx.schedule_event(time, payload);
        }
    }

    async fn handle_input<'a>(
        &'a mut self,
        time: Self::Time,
        inputs: &'a [Self::Input],
        cx: &'a mut UpdateContext<'a, Self>,
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

    async fn handle_scheduled<'a>(
        &'a mut self,
        time: Self::Time,
        payload: &Self::Scheduled,
        cx: &'a mut UpdateContext<'a, Self>,
    ) {
        let record = HandleRecord::Scheduled(time, *payload);
        self.handle_record.push_back(record.clone());

        let state = *cx.get_input_state().await.unwrap();
        cx.emit_event((state, record));
    }

    async fn interpolate<'a>(
        &'a self,
        base_time: Self::Time,
        interpolated_time: Self::Time,
        cx: &InterpolateContext<'a, Self>
    ) -> Self::OutputState {
        self.clone()
    }
}
