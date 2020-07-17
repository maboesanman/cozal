use flume::{Sender, Receiver, unbounded};
use winit::{
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use std::time::Instant;
use crate::core::event::{event_factory::EventFactory, event::{EventTimestamp, EventContent, Event, EventPayload}};

pub struct WinitLoop {
    sender: Sender<Event<winit::event::Event<'static, ()>>>,
    event_loop: winit::event_loop::EventLoop<()>,
}

impl WinitLoop {
    pub fn new() -> (Self, winit::window::Window, Receiver<Event<winit::event::Event<'static, ()>>>) {
        Self::new_from_builder(WindowBuilder::new())
    }

    pub fn new_from_builder(builder: WindowBuilder) -> (Self, winit::window::Window, Receiver<Event<winit::event::Event<'static, ()>>>) {
        let (sender, receiver) = unbounded();
        let event_loop = EventLoop::new();
        let window = builder.build(&event_loop).unwrap();
        (
            WinitLoop {
                sender,
                event_loop,
            },
            window, 
            receiver
        )
    }

    pub fn run(self, ef: &'static EventFactory) -> ! {
        let start = Instant::now();
        let sender = self.sender;

        self.event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;
            if let Some(e) = event.to_static() {
                let t = Instant::now();
                let e = EventContent {
                    timestamp: EventTimestamp {
                        time: t - start,
                        priority: 0,
                    },
                    payload: EventPayload::Payload(e),
                };
                let e = ef.new_event(e);
                sender.send(e).unwrap();
            }
        });
    }
}
