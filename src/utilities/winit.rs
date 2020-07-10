use flume::{Sender, Receiver, unbounded};
use winit::{
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use std::time::Instant;
use crate::core::event::{event_factory::EventFactory, event::{EventTimestamp, EventContent, Event}};

pub struct WinitLoop {
    ef: &'static EventFactory,
    sender: Sender<Event<winit::event::Event<'static, ()>>>,
    event_loop: winit::event_loop::EventLoop<()>,
}

impl WinitLoop {
    pub fn new(ef: &'static EventFactory) -> (Self, winit::window::Window, Receiver<Event<winit::event::Event<'_, ()>>>) {
        let (sender, receiver) = unbounded();
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new().build(&event_loop).unwrap();
        (
            WinitLoop {
                ef,
                sender,
                event_loop,
            },
            window, 
            receiver
        )
    }

    pub fn run(self) -> ! {
        let start = Instant::now();
        let ef = self.ef;
        let sender = self.sender;
        let event_loop = self.event_loop;
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
                let e = ef.new_event(e);
                sender.send(e).unwrap();
            }
        });
    }
}
