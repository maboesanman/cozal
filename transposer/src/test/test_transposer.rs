use rand::Rng;

use crate::context::{
    HandleInputContext,
    HandleScheduleContext,
    InitContext,
    InputStateContextExt,
    InterpolateContext,
};
use crate::{StateRetriever, Transposer, TransposerInput, TransposerInputEventHandler};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HandleRecord {
    Input(usize, usize),
    Scheduled(usize, usize),
}

#[derive(Clone)]
pub(crate) struct TestTransposer {
    init_events: Vec<(usize, usize)>,

    pub handle_record: im::Vector<(HandleRecord, u64)>,
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

pub(crate) struct TestTransposerInput1;
pub(crate) struct TestTransposerInput2;

impl TransposerInput for TestTransposerInput1 {
    // set up with macro
    type Base = TestTransposer;

    type InputEvent = usize;

    type InputState = usize;

    const SORT: u64 = 1;
}

impl TransposerInput for TestTransposerInput2 {
    // set up with macro
    type Base = TestTransposer;

    type InputEvent = usize;

    type InputState = usize;

    const SORT: u64 = 2;
}

// set up with macro
pub(crate) trait TestTransposerStateRetriever:
    StateRetriever<TestTransposerInput1> + StateRetriever<TestTransposerInput2>
{
}

// set up with macro
impl<T> TestTransposerStateRetriever for T where
    T: StateRetriever<TestTransposerInput1> + StateRetriever<TestTransposerInput2>
{
}

// the default handler impl for inputs
impl TransposerInputEventHandler<TestTransposerInput1> for TestTransposer {
    fn can_handle(
        _time: Self::Time,
        _event: &<TestTransposerInput1 as TransposerInput>::InputEvent,
    ) -> bool {
        false
    }
}

impl Transposer for TestTransposer {
    type Time = usize;

    type OutputState = Vec<(HandleRecord, u64)>;

    type Scheduled = usize;

    type OutputEvent = usize;

    // set up with macro
    type InputStateManager = dyn TestTransposerStateRetriever;

    async fn init(&mut self, cx: &mut dyn InitContext<'_, Self>) {
        for (time, payload) in self.init_events.drain(..) {
            let _ = cx.schedule_event(time, payload);
        }
    }

    async fn handle_scheduled(
        &mut self,
        payload: Self::Scheduled,
        cx: &mut dyn HandleScheduleContext<'_, Self>,
    ) {
        let record = HandleRecord::Scheduled(cx.current_time(), payload);
        self.handle_record.push_back((record, cx.get_rng().gen()));

        let state = cx.get_input_state::<TestTransposerInput1>().await;
        cx.emit_event(*state).await;
    }

    async fn interpolate(&self, _cx: &mut dyn InterpolateContext<'_, Self>) -> Self::OutputState {
        self.handle_record.clone().into_iter().collect()
    }
}

impl TransposerInputEventHandler<TestTransposerInput2> for TestTransposer {
    async fn handle_input(&mut self, input: &usize, cx: &mut dyn HandleInputContext<'_, Self>) {
        let record = HandleRecord::Input(cx.current_time(), *input);
        self.handle_record.push_back((record, cx.get_rng().gen()));

        let state = cx.get_input_state::<TestTransposerInput1>().await;

        cx.emit_event(*state).await;
    }
}
