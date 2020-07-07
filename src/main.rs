#[macro_use]
extern crate lazy_static;

use futures::stream::StreamExt;
// use winit::{
//     event::{Event, WindowEvent, DeviceEvent},
//     event_loop::{ControlFlow, EventLoop},
//     window::WindowBuilder,
// };

mod core;
mod example_game;

use crate::example_game::MyUpdater;
use crate::core::event_factory::EventFactory;
use crate::core::game::Game;
use crate::core::debug_sink::DebugSink;

lazy_static! {
    static ref EVENT_FACTORY: EventFactory = EventFactory::new();
}

#[tokio::main]
async fn main() {
    let game: Game<(), (), usize, MyUpdater, ()> = core::game::Game::new(&EVENT_FACTORY);

    let debug = DebugSink {};

    let result = game.forward(debug).await;
    print!("{:?}", result);
    // let event_loop = EventLoop::new();
    // let window = WindowBuilder::new().build(&event_loop).unwrap();

    // event_loop.run(move |event, _, control_flow| {
    //     *control_flow = ControlFlow::Wait;

    //     println!("{:?}", event);
    //     match event {
    //         Event::WindowEvent {
    //             event: WindowEvent::CloseRequested,
    //             window_id,
    //         } => {
    //             if window_id == window.id() {
    //                 *control_flow = ControlFlow::Exit
    //             }
    //         }
    //         Event::DeviceEvent {
    //             event: DeviceEvent::Key(_),
    //             device_id: _device_id,
    //         } =>{
    //             // println!("{:?}", event)
    //         }
    //         _ => {}
    //     }
    // });
}