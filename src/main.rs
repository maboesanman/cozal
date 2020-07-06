use winit::{
    event::{Event, WindowEvent, DeviceEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

mod core;
mod example_game;

fn main() {


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