// pub(self) mod buffered_item;
pub(self) mod engine_time;
pub(self) mod expire_handle_factory;
pub(self) mod frame;
pub(self) mod frame_update;
pub(self) mod input_buffer;
pub(self) mod pin_stack;
pub(self) mod sparse_buffer_stack;

// pub mod lazy_state;

#[cfg(test)]
pub mod test;

// use core::cmp::Ordering;
// use core::future::Future;
// use core::pin::Pin;
// use core::task::{Context, Poll};
// use std::collections::BTreeMap;

// use futures_core::FusedFuture;
// use pin_project::pin_project;

// use self::buffered_item::BufferedItem;
// use self::engine_time::EngineTime;
// use self::input_buffer::InputBuffer;
// use self::sparse_buffer_stack::SparseBufferStack;
// use self::update_item::{DataEmitted, UpdateItem, UpdateItemData};
// use crate::source::{Source, SourcePoll};
// use crate::transposer::Transposer;

// type StateMap<'map, T, const N: usize> =
//     SparseBufferStack<'map, UpdateItem<'map, T>, BufferedItem<'map, T>, N>;

// type OutputBuffer<'map, T> =
//     BTreeMap<EngineTime<'map, <T as Transposer>::Time>, Vec<<T as Transposer>::Output>>;

// /// A struct which implements the [`StatefulScheduleStream`] trait for a [`Transposer`].
// ///
// /// This implementation does the following:
// /// - rollback state and replay to resolve instability in the order of the input stream.
// /// -- this is useful for online multiplayer games, where the network latency can jumble inputs.
// /// - respond to rollback events from the input stream.
// #[pin_project(project=EngineProjection)]
// pub struct TransposerEngine<
//     'map,
//     T: Transposer + Clone + 'map,
//     S: Source<Time = T::Time, Event = T::Input, State = T::InputState>,
//     const N: usize,
// > where
//     T::Scheduled: Clone,
// {
//     #[pin]
//     input_stream: S,

//     input_buffer:  InputBuffer<T::Time, T::Input>,
//     output_buffer: OutputBuffer<'map, T>,

//     #[pin]
//     state_map: StateMap<'map, T, N>,
// }

// impl<
//         'map,
//         T: Transposer + Clone + 'map,
//         S: Source<Time = T::Time, Event = T::Input, State = T::InputState>,
//         const N: usize,
//     > TransposerEngine<'map, T, S, N>
// where
//     T::Scheduled: Clone,
// {
//     /// create a new TransposerEngine, consuming the input stream.
//     pub fn new(
//         input_stream: S,
//         initial_transposer: T,
//         rng_seed: [u8; 32],
//     ) -> TransposerEngine<'map, T, S, N> {
//         let first_state_map_item = UpdateItem::new(
//             EngineTime::Init,
//             UpdateItemData::Init(Box::new(initial_transposer.clone())),
//         );

//         let state_map = SparseBufferStack::new(
//             // pass in the first stack item
//             first_state_map_item,
//             // create the corresponding buffer using a reference to the stack item
//             move |update_item| BufferedItem::new(initial_transposer, update_item, rng_seed),
//         );

//         Self {
//             input_stream,
//             input_buffer: InputBuffer::new(),
//             output_buffer: BTreeMap::new(),
//             state_map,
//         }
//     }

//     fn pop_output_buffer(
//         output_buffer: &mut OutputBuffer<'map, T>,
//         poll_time: T::Time,
//     ) -> Option<(T::Time, T::Output)> {
//         let mut entry = output_buffer.first_entry()?;
//         let time = entry.key().raw_time();
//         if time > poll_time {
//             return None
//         }

//         let vec = entry.get_mut();
//         let out = vec.pop().unwrap();
//         if vec.is_empty() {
//             output_buffer.pop_first();
//         }
//         Some((time, out))
//     }

//     fn push_output_buffer(
//         output_buffer: &mut OutputBuffer<'map, T>,
//         time: EngineTime<'map, T::Time>,
//         outputs: Vec<T::Output>,
//     ) {
//         if output_buffer.insert(time, outputs).is_some() {
//             // enginetimes are supposed to be unique, so if these are the same something bad has happened.
//             unreachable!()
//         }
//     }

//     fn output_buffer_first_time<'a>(
//         output_buffer: &'a OutputBuffer<'map, T>,
//     ) -> Option<&'a EngineTime<'map, T::Time>> {
//         let (key, _) = output_buffer.first_key_value()?;
//         Some(key)
//     }

//     /// this function responds to a rollback event
//     ///
//     /// returns the time to emit a rollback for, if needed.
//     fn handle_input_rollback(
//         input_buffer: &mut InputBuffer<T::Time, T::Input>,
//         mut state_map: Pin<&mut StateMap<'map, T, N>>,
//         output_buffer: &mut OutputBuffer<'map, T>,
//         time: T::Time,
//     ) -> Option<T::Time> {
//         input_buffer.rollback(time);

//         let mut rollback_needed = None;
//         loop {
//             let last_state_map_time = state_map.as_mut().peek().time.raw_time();

//             if time > last_state_map_time {
//                 break
//             }

//             if state_map.as_ref().can_pop() {
//                 let state_map_ref = state_map.as_ref();
//                 let update_item = state_map_ref.peek();
//                 let events_emitted: DataEmitted<T::Time> = update_item.data_emitted();
//                 if events_emitted.any() {
//                     rollback_needed = match events_emitted {
//                         DataEmitted::Event => Some(last_state_map_time),
//                         DataEmitted::State(t) => Some(t),
//                         _ => unreachable!(),
//                     };

//                     // throw away everything at or after the discarded frame.
//                     output_buffer.split_off(&update_item.time);
//                 }
//                 state_map.as_mut().pop();
//             } else {
//                 break
//             }
//         }

//         // now check to see if the last frame (which was handled before the rollback)
//         // was used to generate a state. To be perfect here we would need to store every state call,
//         // so we just assume if any state was emitted in this range that a rollback is needed at
//         // exactly the input rollback time.
//         if matches!(
//             state_map.as_mut().peek().data_emitted(),
//             DataEmitted::State(_)
//         ) {
//             rollback_needed = Some(time)
//         }

//         rollback_needed
//     }

//     /// this function sets up the gurantee that:
//     /// self.input_buffer.first_time() > self.state_map.peek().time
//     ///
//     /// returns the time to emit a rollback for, if needed.
//     fn resolve_state_map_and_input_buffer(
//         input_buffer: &mut InputBuffer<T::Time, T::Input>,
//         mut state_map: Pin<&mut StateMap<'map, T, N>>,
//         output_buffer: &mut OutputBuffer<'map, T>,
//     ) -> Option<T::Time> {
//         let mut rollback_needed = None;
//         let next_input_time = match input_buffer.first_time() {
//             Some(t) => t,
//             None => return None, // no inputs means we're good here.
//         };
//         loop {
//             let last_state_map_time = state_map.as_mut().peek().time.raw_time();

//             if next_input_time > last_state_map_time {
//                 break
//             }

//             // SAFETY: the references in update_item.time are dropped before pop is called again, so they are never invalid.
//             match unsafe { state_map.as_mut().pop_recover() } {
//                 Some(update_item) => {
//                     if update_item.data_emitted().any() {
//                         rollback_needed = Some(last_state_map_time);

//                         // throw away everything at or after the discarded frame.
//                         output_buffer.split_off(&update_item.time);
//                     }
//                     match update_item.data {
//                         UpdateItemData::Init(_) => unreachable!(),
//                         UpdateItemData::Input(inputs) => {
//                             let time = match update_item.time {
//                                 EngineTime::Input(t) => t,
//                                 _ => unreachable!(),
//                             };
//                             input_buffer.extend_front(time, inputs);
//                         },
//                         UpdateItemData::Schedule => {},
//                     }
//                 },
//                 None => break,
//             }
//         }

//         rollback_needed
//     }

//     fn poll_internal<F, Out>(
//         mut self: Pin<&mut Self>,
//         poll_time: T::Time,
//         cx: &mut Context<'_>,
//         state_builder: F,
//     ) -> SourcePoll<T::Time, T::Output, Out, <Self as Source>::Error>
//     where
//         F: Fn(
//             Pin<&mut S>,
//             Pin<&mut StateMap<'map, T, N>>,
//             usize,
//             &mut Context,
//         ) -> PollInternal<T, Out>,
//     {
//         let this = self.as_mut().project();
//         let mut input_stream: Pin<&mut S> = this.input_stream;
//         let input_buffer: &mut InputBuffer<T::Time, T::Input> = this.input_buffer;
//         let output_buffer: &mut OutputBuffer<'map, T> = this.output_buffer;
//         let mut state_map: Pin<&mut StateMap<'map, T, N>> = this.state_map;

//         let mut input_state_event_update = InputStateEventUpdate::<T>::None;

//         'main: loop {
//             // resolve events and rollbacks from previous iteration.
//             match input_state_event_update {
//                 InputStateEventUpdate::None => {},
//                 InputStateEventUpdate::Event(time, input) => {
//                     input_buffer.insert_back(time, input);
//                     Self::resolve_state_map_and_input_buffer(
//                         input_buffer,
//                         state_map.as_mut(),
//                         output_buffer,
//                     );
//                     input_state_event_update = InputStateEventUpdate::None;
//                 },
//                 InputStateEventUpdate::Rollback(time) => {
//                     let rollback_time = Self::handle_input_rollback(
//                         input_buffer,
//                         state_map.as_mut(),
//                         output_buffer,
//                         time,
//                     );
//                     if let Some(rollback_time) = rollback_time {
//                         break 'main SourcePoll::Rollback(rollback_time)
//                     }
//                     Self::resolve_state_map_and_input_buffer(
//                         input_buffer,
//                         state_map.as_mut(),
//                         output_buffer,
//                     );
//                     input_state_event_update = InputStateEventUpdate::None;
//                 },
//             }

//             let last_buffered_index_before_poll = state_map
//                 .as_mut()
//                 .last_buffered_index_by(poll_time, |x| x.time.raw_time());
//             let last_index = state_map.len() - 1;
//             let previously_processed_future_update = last_index != last_buffered_index_before_poll;
//             let mut has_next_update =
//                 if let Some(next_update) = state_map.get(last_buffered_index_before_poll + 1) {
//                     next_update.time.raw_time() <= poll_time
//                 } else {
//                     false
//                 };

//             let update_and_buffer = state_map
//                 .as_mut()
//                 .get_pinned_mut(last_buffered_index_before_poll);
//             let update_and_buffer = update_and_buffer.unwrap();

//             let update: &UpdateItem<'map, T> = update_and_buffer.0;
//             let mut buffer: Pin<&mut BufferedItem<'map, T>> = update_and_buffer.1.unwrap();

//             // if we have a buffered output before our update, emit that.
//             if let Some(output_time) = Self::output_buffer_first_time(output_buffer) {
//                 if output_time < &update.time {
//                     match input_stream
//                         .as_mut()
//                         .poll_events(output_time.raw_time(), cx)
//                     {
//                         SourcePoll::Pending => break 'main SourcePoll::Pending,
//                         SourcePoll::Rollback(time) => {
//                             input_state_event_update = InputStateEventUpdate::Rollback(time);
//                             continue 'main
//                         },
//                         SourcePoll::Event(input, time) => {
//                             input_state_event_update = match T::can_handle(time, &input) {
//                                 true => InputStateEventUpdate::Event(time, input),
//                                 false => InputStateEventUpdate::None,
//                             };
//                             continue 'main
//                         },
//                         _ => {},
//                     }

//                     let (time, input) = Self::pop_output_buffer(output_buffer, poll_time).unwrap();
//                     break 'main SourcePoll::Event(input, time)
//                 }
//             }

//             // try to advance the first dependent buffer to complete
//             if !buffer.is_terminated() {
//                 // verify there are no new events or rollbacks before proceeding
//                 // if we need a state, obtain it and set buffer.input_state
//                 match buffer.input_state.requested() {
//                     true => {
//                         let state = match input_stream.as_mut().poll(update.time.raw_time(), cx) {
//                             SourcePoll::Pending => break 'main SourcePoll::Pending,
//                             SourcePoll::Rollback(time) => {
//                                 input_state_event_update = InputStateEventUpdate::Rollback(time);
//                                 continue 'main
//                             },
//                             SourcePoll::Event(input, time) => {
//                                 input_state_event_update = match T::can_handle(time, &input) {
//                                     true => InputStateEventUpdate::Event(time, input),
//                                     false => InputStateEventUpdate::None,
//                                 };
//                                 continue 'main
//                             },
//                             SourcePoll::Scheduled(state, ..) => state,
//                             SourcePoll::Ready(state) => state,
//                         };

//                         // pass in our new state.
//                         match buffer.as_mut().project().input_state.set(state) {
//                             Err(_) => unreachable!(),
//                             Ok(()) => {},
//                         }
//                     },
//                     false => {
//                         match input_stream
//                             .as_mut()
//                             .poll_events(update.time.raw_time(), cx)
//                         {
//                             SourcePoll::Pending => break 'main SourcePoll::Pending,
//                             SourcePoll::Rollback(time) => {
//                                 input_state_event_update = InputStateEventUpdate::Rollback(time);
//                                 continue 'main
//                             },
//                             SourcePoll::Event(input, time) => {
//                                 input_state_event_update = match T::can_handle(time, &input) {
//                                     true => InputStateEventUpdate::Event(time, input),
//                                     false => InputStateEventUpdate::None,
//                                 };
//                                 continue 'main
//                             },
//                             SourcePoll::Scheduled((), ..) => {},
//                             SourcePoll::Ready(()) => {},
//                         }
//                     },
//                 };

//                 // poll the actual update.
//                 match buffer.as_mut().poll(cx) {
//                     Poll::Ready(outputs) => {
//                         // we do not want to do anything with events if they have already been emitted.
//                         if update.events_emitted() {
//                             continue 'main
//                         }

//                         if outputs.is_empty() {
//                             update.mark_none_emitted();
//                             continue 'main
//                         }

//                         update.mark_event_emitted();
//                         Self::push_output_buffer(output_buffer, update.time.clone(), outputs);
//                         let (time, input) =
//                             Self::pop_output_buffer(output_buffer, poll_time).unwrap();
//                         break 'main SourcePoll::Event(input, time)
//                     },
//                     Poll::Pending => {
//                         if buffer.input_state.requested() {
//                             continue 'main
//                         } else {
//                             break 'main SourcePoll::Pending
//                         }
//                     },
//                 }
//             }

//             // now buffer is terminated.
//             // we need to determine which of the following to do:
//             //  - poll input at poll_time, and perform interpolation
//             //  - buffer an existing update
//             //  - create a new update and buffer it

//             // we are at the end of the chain
//             if !previously_processed_future_update {
//                 if let Some(update_item) = buffer.next_update_item(input_buffer) {
//                     if update_item.time.raw_time() <= poll_time {
//                         has_next_update = true;
//                     }
//                     state_map.as_mut().push(|_| update_item);
//                 }
//             }

//             if has_next_update {
//                 state_map
//                     .as_mut()
//                     .buffer(
//                         last_buffered_index_before_poll + 1,
//                         BufferedItem::dup,
//                         BufferedItem::refurb,
//                     )
//                     .unwrap();
//                 continue 'main
//             } else {
//                 let next_possible_scheduled_event_time = state_map
//                     .find(|update_item| !update_item.data_emitted().done())
//                     .map(|update_item| update_item.time.raw_time());

//                 match state_builder(
//                     input_stream.as_mut(),
//                     state_map.as_mut(),
//                     last_buffered_index_before_poll,
//                     cx,
//                 ) {
//                     PollInternal::BreakPending => break 'main SourcePoll::Pending,
//                     PollInternal::Continue {
//                         input_state_event_update: new_update,
//                     } => {
//                         input_state_event_update = new_update;
//                         continue 'main
//                     },
//                     PollInternal::Ready {
//                         interpolated,
//                         scheduled_input_time,
//                     } => {
//                         break 'main match (scheduled_input_time, next_possible_scheduled_event_time)
//                         {
//                             (None, None) => SourcePoll::Ready(interpolated),
//                             (None, Some(t)) => SourcePoll::Scheduled(interpolated, t),
//                             (Some(t), None) => SourcePoll::Scheduled(interpolated, t),
//                             (Some(t1), Some(t2)) => match t1.cmp(&t2) {
//                                 Ordering::Less => SourcePoll::Scheduled(interpolated, t1),
//                                 Ordering::Equal => SourcePoll::Scheduled(interpolated, t1),
//                                 Ordering::Greater => SourcePoll::Scheduled(interpolated, t2),
//                             },
//                         }
//                     },
//                 }
//             }
//         }
//     }

//     fn poll_forget_internal<const FORGET: bool>(
//         self: Pin<&mut Self>,
//         poll_time: T::Time,
//         cx: &mut Context<'_>,
//     ) -> SourcePoll<T::Time, T::Output, T::OutputState, ()> {
//         self.poll_internal(
//             poll_time,
//             cx,
//             |mut input_stream, mut state_map, last_buffered_index_before_poll, cx| {
//                 // get our state to interpolate with
//                 let poll = match FORGET {
//                     true => input_stream.as_mut().poll_forget(poll_time, cx),
//                     false => input_stream.as_mut().poll(poll_time, cx),
//                 };

//                 let (state, scheduled_input_time) = match poll {
//                     SourcePoll::Pending => return PollInternal::BreakPending,
//                     SourcePoll::Rollback(time) => {
//                         return PollInternal::Continue {
//                             input_state_event_update: InputStateEventUpdate::Rollback(time),
//                         }
//                     },
//                     SourcePoll::Event(input, time) => {
//                         return PollInternal::Continue {
//                             input_state_event_update: match T::can_handle(time, &input) {
//                                 true => InputStateEventUpdate::Event(time, input),
//                                 false => InputStateEventUpdate::None,
//                             },
//                         }
//                     },
//                     SourcePoll::Scheduled(state, time) => (state, Some(time)),
//                     SourcePoll::Ready(state) => (state, None),
//                 };

//                 // get the buffer and update
//                 let update_and_buffer = state_map
//                     .as_mut()
//                     .get_pinned_mut(last_buffered_index_before_poll);
//                 let update_and_buffer = update_and_buffer.unwrap();

//                 let update: &UpdateItem<'map, T> = update_and_buffer.0;
//                 let buffer: Pin<&mut BufferedItem<'map, T>> = update_and_buffer.1.unwrap();

//                 // construct the interpolated value
//                 let base_time = update.time.raw_time();
//                 let interpolated = buffer
//                     .transposer_frame
//                     .transposer
//                     .interpolate(base_time, poll_time, state);

//                 if !FORGET {
//                     update.mark_state_emitted(poll_time);
//                 }

//                 PollInternal::Ready {
//                     interpolated,
//                     scheduled_input_time,
//                 }
//             },
//         )
//     }
// }

// enum PollInternal<T: Transposer, Out> {
//     BreakPending,
//     Continue {
//         input_state_event_update: InputStateEventUpdate<T>,
//     },
//     Ready {
//         interpolated:         Out,
//         scheduled_input_time: Option<T::Time>,
//     },
// }

// enum InputStateEventUpdate<T: Transposer> {
//     None,
//     Event(T::Time, T::Input),
//     Rollback(T::Time),
// }

// impl<
//         'map,
//         T: Transposer + Clone + 'map,
//         S: Source<Time = T::Time, Event = T::Input, State = T::InputState>,
//         const N: usize,
//     > Source for TransposerEngine<'map, T, S, N>
// where
//     T::Scheduled: Clone,
// {
//     type Time = T::Time;
//     type Event = T::Output;
//     type State = T::OutputState;

//     fn poll(
//         self: Pin<&mut Self>,
//         poll_time: Self::Time,
//         cx: &mut Context<'_>,
//     ) -> SourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
//         self.poll_forget_internal::<false>(poll_time, cx)
//     }

//     fn poll_forget(
//         self: Pin<&mut Self>,
//         poll_time: Self::Time,
//         cx: &mut Context<'_>,
//     ) -> SourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
//         self.poll_forget_internal::<true>(poll_time, cx)
//     }

//     fn poll_events(
//         self: Pin<&mut Self>,
//         poll_time: Self::Time,
//         cx: &mut Context<'_>,
//     ) -> SourcePoll<Self::Time, Self::Event, (), Self::Error> {
//         self.poll_internal(
//             poll_time,
//             cx,
//             |mut input_stream, _state_map, _last_buffered_index_before_poll, cx| {
//                 // get our state to interpolate with
//                 let scheduled_input_time = match input_stream.as_mut().poll_events(poll_time, cx) {
//                     SourcePoll::Pending => return PollInternal::BreakPending,
//                     SourcePoll::Rollback(time) => {
//                         return PollInternal::Continue {
//                             input_state_event_update: InputStateEventUpdate::Rollback(time),
//                         }
//                     },
//                     SourcePoll::Event(input, time) => {
//                         return PollInternal::Continue {
//                             input_state_event_update: match T::can_handle(time, &input) {
//                                 true => InputStateEventUpdate::Event(time, input),
//                                 false => InputStateEventUpdate::None,
//                             },
//                         }
//                     },
//                     SourcePoll::Scheduled((), time) => Some(time),
//                     SourcePoll::Ready(()) => None,
//                 };

//                 PollInternal::Ready {
//                     interpolated: (),
//                     scheduled_input_time,
//                 }
//             },
//         )
//     }
// }
