#[macro_use]
extern crate lazy_static;

use futures::future::ready;
use futures::stream::StreamExt;
use std::thread;
use tokio::runtime::Runtime;

use crate::example_game::ExampleTransposer;
use crate::core::event::event::{Event, EventContent};
use crate::core::event::event_factory::EventFactory;
use crate::core::transposer::transposer_stream::Game;
use crate::utilities::debug_sink::DebugSink;
use utilities::winit::WinitLoop;

mod core;
mod example_game;
mod utilities;

lazy_static! {
    static ref EVENT_FACTORY: EventFactory = EventFactory::new();
}

fn main() {
    let (winit, _, receiver) = WinitLoop::new(&EVENT_FACTORY);

    thread::spawn(move || {
        let key_presses = receiver.filter_map(move |e: Event<winit::event::Event<'_, ()>>| {
            ready(match e.content.payload {
                winit::event::Event::WindowEvent {
                    window_id: _,
                    event,
                } => match event {
                    winit::event::WindowEvent::ReceivedCharacter(_) => {
                        let event = EventContent {
                            timestamp: e.content.timestamp,
                            payload: (),
                        };
                        let event = EVENT_FACTORY.new_event(event);
                        Some(event)
                    }
                    _ => None,
                },
                _ => None,
            })
        });

        let game: Game<ExampleTransposer, _> = Game::new(key_presses, &EVENT_FACTORY);
        let game = game.map(move |event| Ok(event));
        let mut rt = Runtime::new().unwrap();
        let fut1 = game.forward(DebugSink::new());
        rt.block_on(fut1).unwrap();
    });

    winit.run();
}
