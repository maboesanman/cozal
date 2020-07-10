#[macro_use]
extern crate lazy_static;

use crate::core::event::EventContent;
use flume::unbounded;
use futures::future::ready;
use futures::stream::StreamExt;
use std::thread;
use tokio::runtime::Runtime;
use tokio::time::Instant;
use winit::{
    // event::{Event, WindowEvent, DeviceEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

mod core;
mod example_game;

use crate::core::debug_sink::DebugSink;
use crate::core::event::*;
use crate::core::event_factory::EventFactory;
use crate::core::game::Game;
use crate::example_game::MyUpdater;

lazy_static! {
    static ref EVENT_FACTORY: EventFactory = EventFactory::new();
}

fn main() {
    let (sender, receiver) = unbounded();

    thread::spawn(move || {
        let key_presses = receiver.filter_map(move |e: Event<winit::event::Event<'_, ()>>| {
            // todo!()
            // print!("{:?}", e.content.payload);
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
            // Ok(Some(e: Event<()>))
        });

        let game: Game<MyUpdater, _> = Game::new(key_presses, &EVENT_FACTORY);
        let game = game.map(move |event| Ok(event));
        let mut rt = Runtime::new().unwrap();
        let fut1 = game.forward(DebugSink::new());
        rt.block_on(fut1).unwrap();
    });

    let event_loop = EventLoop::new();
    let _window = WindowBuilder::new().build(&event_loop).unwrap();
    let start = Instant::now();
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Some(e) = event.to_static() {
            let t = Instant::now();
            let e = EventContent {
                timestamp: EventTimestamp {
                    time: t - start,
                    priority: 0,
                },
                payload: e,
            };
            let e = EVENT_FACTORY.new_event(e);
            sender.send(e).unwrap();
        }
    });
}
