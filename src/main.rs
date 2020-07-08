#[macro_use]
extern crate lazy_static;

use futures::stream::StreamExt;
use winit::{
    // event::{Event, WindowEvent, DeviceEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use tokio::runtime::Runtime;
use std::thread;

mod core;
mod example_game;

use crate::example_game::MyUpdater;
use crate::core::event_factory::EventFactory;
use crate::core::game::Game;
use crate::core::debug_sink::DebugSink;
use crate::core::channel_stream::channel_stream;

lazy_static! {
    static ref EVENT_FACTORY: EventFactory = EventFactory::new();
}

fn main() {
    let game: Game<(), (), usize, MyUpdater, ()> = core::game::Game::new(&EVENT_FACTORY);

    let fut1 = game.forward(DebugSink::new());
    // print!("{:?}", result);

    let (sink, stream) = channel_stream();
    let fut2 = stream.forward(DebugSink::new());

    thread::spawn(move || {
        let mut rt = Runtime::new().unwrap();
        rt.block_on(fut2);
    });

    let event_loop = EventLoop::new();
    let _window = WindowBuilder::new().build(&event_loop).unwrap();
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Some(e) = event.to_static() {
            sink.enque(e);
        }
    });
}