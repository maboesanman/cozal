#![feature(async_fn_in_trait)]

use std::time::{Duration, Instant};

use cozal::sources::no_input_transposer::NoInputTransposerSource;
use cozal::traits::SourceExt;
use futures::StreamExt;
use transposer::context::{HandleScheduleContext, InitContext, InterpolateContext};
use transposer::step::NoInputManager;
use transposer::Transposer;

#[derive(Clone)]
pub(crate) struct CollatzTransposer {
    value: usize,
}

impl CollatzTransposer {
    pub fn new(value: usize) -> Self {
        Self {
            value,
        }
    }
}

impl Transposer for CollatzTransposer {
    type Time = Instant;

    type OutputState = ();

    type Scheduled = ();

    type OutputEvent = usize;

    // set up with macro
    type InputStateManager = NoInputManager;

    async fn init(&mut self, cx: &mut dyn InitContext<'_, Self>) {
        cx.schedule_event(cx.current_time(), ()).unwrap();
    }

    async fn handle_scheduled(
        &mut self,
        _payload: Self::Scheduled,
        cx: &mut dyn HandleScheduleContext<'_, Self>,
    ) {
        cx.emit_event(self.value).await;

        if self.value % 2 == 0 {
            self.value = self.value / 2;
        } else {
            self.value = self.value * 3 + 1;
        }

        cx.schedule_event(cx.current_time() + Duration::from_millis(100), ())
            .unwrap();
    }

    async fn interpolate(&self, _cx: &mut dyn InterpolateContext<'_, Self>) -> Self::OutputState {}
}

#[tokio::test]
async fn test() {
    let transposer = CollatzTransposer::new(27);

    let source = NoInputTransposerSource::new(transposer, Instant::now(), [0; 32]);

    let stream = source.interrupt_stream(|i| tokio::time::sleep_until(i.into()));

    stream
        .for_each(|(_, i)| {
            let e = match i {
                cozal::source_poll::Interrupt::Event(e) => e,
                cozal::source_poll::Interrupt::FinalizedEvent(e) => e,
                _ => panic!(),
            };

            println!("{:?}", e);

            async { () }
        })
        .await;
}
