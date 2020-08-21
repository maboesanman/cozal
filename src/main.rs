use futures::stream::StreamExt;
use std::{time::{Instant, Duration}};

use crate::core::transposer::transposer_engine::TransposerEngine;
use crate::example_game::{get_filtered_stream, ExampleTransposer};
use crate::utilities::debug_sink::DebugSink;
use utilities::winit::WinitLoop;
use crate::core::event::event_timestamp::EventTimestamp;

mod core;
mod example_game;
mod utilities;

#[tokio::main]
async fn main() {
    let (winit, window, receiver) = WinitLoop::new();
    window.set_visible(true);

    let key_presses = get_filtered_stream(Instant::now(), receiver);
    let game: TransposerEngine<ExampleTransposer, _> = TransposerEngine::new(key_presses).await;
    
    let poll = game.poll(Duration::from_secs(10));
    tokio::spawn(async move {
        let poll = poll.map(move |event| Ok(event));
        poll.forward(DebugSink::new()).await.unwrap();
    });

    winit.run();
}
