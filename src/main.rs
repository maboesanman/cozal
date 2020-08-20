#[macro_use]
extern crate lazy_static;

use futures::future::ready;
use futures::stream::StreamExt;
use std::{time::Duration, thread};
use tokio::runtime::Runtime;

use crate::core::event::event::{Event, EventPayload};
use crate::core::transposer::transposer_engine::TransposerEngine;
use crate::example_game::{get_filtered_stream, ExampleTransposer};
use crate::utilities::debug_sink::DebugSink;
use utilities::winit::WinitLoop;
use crate::core::event::event_timestamp::EventTimestamp;

mod core;
mod example_game;
mod utilities;

// lazy_static! {
//     static ref EVENT_FACTORY: EventFactory = EventFactory::new();
// }
#[tokio::main]
async fn main() {
    let (winit, window, receiver) = WinitLoop::new();
    window.set_visible(true);

    let key_presses = get_filtered_stream(receiver);

    tokio::spawn(async move {
        let game: TransposerEngine<ExampleTransposer, _> = TransposerEngine::new(key_presses).await;
        let poll = game.poll(EventTimestamp {
            time: Duration::from_secs(10),
            priority: 0,
        });
        let poll = poll.map(move |event| Ok(event));
        let mut rt = Runtime::new().unwrap();
        poll.forward(DebugSink::new()).await;
    });

    winit.run();
}
