use crate::core::Event;
// use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use flume::{unbounded, Receiver, Sender};
use std::time::Instant;
use winit::{
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

// todo document.
pub struct WinitLoop {
    sender: Sender<Event<Instant, winit::event::Event<'static, ()>>>,
    event_loop: winit::event_loop::EventLoop<()>,
}

impl WinitLoop {
    pub fn new() -> (
        Self,
        winit::window::Window,
        Receiver<Event<Instant, winit::event::Event<'static, ()>>>,
    ) {
        Self::new_from_builder(WindowBuilder::new())
    }

    pub fn new_from_builder(
        builder: WindowBuilder,
    ) -> (
        Self,
        winit::window::Window,
        Receiver<Event<Instant, winit::event::Event<'static, ()>>>,
    ) {
        let (sender, receiver) = unbounded();
        let event_loop = EventLoop::new();
        let window = builder.build(&event_loop).unwrap();
        (Self { sender, event_loop }, window, receiver)
    }

    pub fn run(self) -> ! {
        let sender = self.sender;
        self.event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;
            if let Some(e) = event.to_static() {
                let e = Event {
                    timestamp: Instant::now(),
                    payload: e,
                };
                if let Err(e) = sender.send(e) {
                    println!("{:?}", e);
                    *control_flow = ControlFlow::Exit;
                }
            }
        });
    }
}
