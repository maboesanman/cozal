use futures::stream::{StreamExt};
use std::time::Instant;
use utilities::{debug_stream::DebugStream, winit::WinitLoop, debug_future::DebugFuture, debug_sink::PrintSink};

use crate::core::schedule_stream::schedule_stream_ext::ScheduleStreamExt;
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
    let game = TransposerEngine::<ExampleTransposer, _>::new(key_presses).await;
    let stream = game.to_realtime(Instant::now());
    let stream = stream.map(move |event| Ok(event));
    // let stream = DebugStream::new(stream, "str");
    let sink = PrintSink::new();
    // let sink = DebugSink::new(sink, "sink");
    let fut = stream.forward(sink);
    // let fut = DebugFuture::new(fut, "fut");

    tokio::spawn(fut);

    winit.run();
}
