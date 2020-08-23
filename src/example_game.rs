// use crate::core::{
//     event::event::{Event, RollbackPayload},
//     transposer::{
//         transposer::{InitResult, Transposer, UpdateResult},
//         transposer_context::TransposerContext,
//         transposer_event::TransposerEvent,
//     },
// };
// use async_trait::async_trait;
// use std::time::{Instant, Duration};
// use futures::{StreamExt, Stream};
// use futures::future::ready;

// #[derive(Clone)]
// pub struct ExampleTransposer {
//     count: usize,
// }

// impl ExampleTransposer {
//     fn handle_external(&self, _event: &Event<Duration, ()>) -> UpdateResult<Self> {
//         let mut new_updater = self.clone();
//         new_updater.count -= 1;
//         UpdateResult {
//             new_updater: Some(new_updater),
//             expired_events: vec![],
//             new_events: vec![],
//             emitted_events: vec![self.count],
//         }
//     }

//     fn handle_internal(&self, event: &Event<Duration, ()>) -> UpdateResult<Self> {
//         let mut new_updater = self.clone();
//         new_updater.count += 1;
//         let new_in_event = Event {
//             timestamp: event.timestamp + Duration::from_secs(1),
//             payload: RollbackPayload::Payload(()),
//         };
//         UpdateResult {
//             new_updater: Some(new_updater),
//             expired_events: vec![],
//             new_events: vec![new_in_event],
//             emitted_events: vec![self.count],
//         }
//     }
// }

// #[async_trait]
// impl Transposer for ExampleTransposer {
//     type Time = Duration;
//     type External = ();
//     type Internal = ();
//     type Out = usize;

//     async fn init(_cx: &TransposerContext) -> InitResult<Self> {
//         InitResult {
//             new_updater: ExampleTransposer { count: 0 },
//             new_events: vec![Event {
//                 timestamp: Duration::from_secs(0),
//                 payload: (),
//             }],
//             emitted_events: vec![],
//         }
//     }
//     async fn update<'a>(
//         &'a self,
//         _cx: &TransposerContext,
//         event: &'a TransposerEvent<Self>,
//     ) -> UpdateResult<Self> {
//         match event {
//             TransposerEvent::External(event) => self.handle_external(&event.event),
//             TransposerEvent::Internal(event) => self.handle_internal(&event.event),
//         }
//     }
// }

// pub fn get_filtered_stream<S: Stream<Item = Event<Instant, winit::event::Event<'static, ()>>>>(
//     start_time: Instant,
//     stream: S,
// ) -> impl Stream<Item = Event<Duration, RollbackPayload<()>>> {
//     stream.filter_map(
//         move |e: Event<Instant, winit::event::Event<'_, ()>>| ready(match e.payload {
//             winit::event::Event::WindowEvent {
//                 window_id: _,
//                 event,
//             } => match event {
//                 winit::event::WindowEvent::ReceivedCharacter(_) => {
//                     let event = Event {
//                         timestamp: e.timestamp - start_time,
//                         payload: RollbackPayload::Payload(()),
//                     };
//                     Some(event)
//                 }
//                 _ => None,
//             },
//             _ => None,
//         }),
//     )
// }
