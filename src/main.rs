use futures::stream::StreamExt;
use std::time::Instant;
use utilities::winit::WinitLoop;

use crate::core::schedule_stream::schedule_stream::ScheduleStreamExt;
use crate::core::transposer::transposer_engine::TransposerEngine;
use crate::example_game::{get_filtered_stream, ExampleTransposer};
use crate::utilities::debug_sink::DebugSink;

mod core;
mod example_game;
mod utilities;

#[tokio::main]
async fn main() {
    let (winit, window, receiver) = WinitLoop::new();
    window.set_visible(true);

    let key_presses = get_filtered_stream(Instant::now(), receiver);
    let game: TransposerEngine<ExampleTransposer, _> = TransposerEngine::new(key_presses).await;
    let stream = game.to_realtime(Instant::now());
    let stream = stream.map(move |event| Ok(event));
    let fut = stream.forward(DebugSink::new());
    tokio::spawn(fut);

    winit.run();
}
