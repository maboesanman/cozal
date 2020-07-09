#[macro_use]
extern crate lazy_static;

use crate::core::event::EventContent;
use futures::stream::{Stream, StreamExt, TryStreamExt};
use winit::{
    // event::{Event, WindowEvent, DeviceEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use tokio::runtime::Runtime;
use tokio::time::{Instant};
use std::thread;
use flume::unbounded;


mod core;
mod example_game;

use crate::example_game::MyUpdater;
use crate::core::event::*;
use crate::core::event_factory::EventFactory;
use crate::core::game::Game;
use crate::core::debug_sink::DebugSink;

lazy_static! {
    static ref EVENT_FACTORY: EventFactory = EventFactory::new();
}

fn main() {
    let (sender, receiver) = unbounded();
    
    // thread::spawn(move || {
    //     let space_presses = receiver.filter_map(|e: Event<winit::event::Event<'_, ()>>| async move {
    //         // todo!()
    //         None
    //         // match e.content.payload {
    //         //     // winit::event::Event::WindowEvent { 
    //         //     //     window_id,
    //         //     //     event,
    //         //     // } => match event {

    //         //     //     Ok(None)
    //         //     // },
    //         //     winit::event::Event::DeviceEvent {
    //         //         device_id,
    //         //         event,
    //         //     } => {
    //         //         match event {
    //         //             winit::event::DeviceEvent::Key(_) => Ok(Some(())),
    //         //             _ => Ok(None)
    //         //         }
    //         //     }
    //         //     _ => Ok(None)
    //         // }
    //         // Ok(Some(e: Event<()>))
    //     });

    //     let game: Game<MyUpdater, _> = Game::new(space_presses, &EVENT_FACTORY);
    //     let mut rt = Runtime::new().unwrap();
    //     let fut1 = game.forward(DebugSink::new());
    //     rt.block_on(fut1);
    // });

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
                    priority: 0
                },
                payload: e,
            };
            let e = EVENT_FACTORY.new_event(e);
            sender.send(e);
        }
    });
}