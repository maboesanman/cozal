use core::pin::Pin;
use std::{collections::BTreeMap, marker::PhantomPinned, process::Output, task::Poll};
use futures::{Future, future::FusedFuture, task::Context};
use pin_project::pin_project;


use crate::core::{Transposer, event_state_stream::{EventStatePoll, EventStateStream}};

use super::{engine_time::EngineTime, input_buffer::InputBuffer, lazy_state::LazyState, sparse_buffer_stack::SparseBufferStack, state_map::{BufferedItem, UpdateItem, UpdateItemData}, transposer_frame::TransposerFrame, transposer_update::TransposerUpdate}; //, state_map::StateMap};

type StateMap<
    'map,
    T: Transposer + Clone + 'map,
    const N: usize
> = SparseBufferStack<'map, UpdateItem<'map, T>, BufferedItem<'map, T>, N>;

type OutputBuffer<'map, T: Transposer + Clone + 'map> = BTreeMap<EngineTime<'map, T::Time>, Vec<T::Output>>;

/// A struct which implements the [`StatefulScheduleStream`] trait for a [`Transposer`].
///
/// This implementation does the following:
/// - rollback state and replay to resolve instability in the order of the input stream.
/// -- this is useful for online multiplayer games, where the network latency can jumble inputs.
/// - respond to rollback events from the input stream.
/// - record the input events for the purpose of storing replay data.
#[pin_project(project=EngineProjection)]
pub struct TransposerEngine<
    'map,
    T: Transposer + Clone + 'map,
    S: EventStateStream<Time = T::Time, Event = T::Input, State = T::InputState>,
    const N: usize
>
where
    T::Scheduled: Clone
{
    #[pin]
    input_stream: S,

    input_buffer: InputBuffer<T::Time, T::Input>,
    output_buffer: OutputBuffer<'map, T>,

    #[pin]
    state_map: StateMap<'map, T, N>,
}

impl<
    'map,
    T: Transposer + Clone + 'map,
    S: EventStateStream<Time = T::Time, Event = T::Input, State = T::InputState>,
    const N: usize,
> TransposerEngine<'map, T, S, N>
    where T::Scheduled: Clone 
{
    /// create a new TransposerEngine, consuming the input stream.
    pub fn new(input_stream: S, initial_transposer: T) -> TransposerEngine<'map, T, S, N> {
        let first_state_map_item = UpdateItem {
            time: EngineTime::Init,
            data: UpdateItemData::Init(Box::new(initial_transposer)),
            events_emitted: false,
        };

        // let state_map = SparseBufferStack::new(
        //     // pass in the first stack item
        //     first_state_map_item,
        //     // create the corresponding buffer using a reference to the stack item
        //     BufferedItem::construct_init,
        //     // perform any initialization requiring a pin to the buffer
        //     |buffer_item_ref, _| buffer_item_ref.init_init()
        // );

        // Self {
        //     input_stream,
        //     input_buffer: InputBuffer::new(),
        //     output_buffer: BTreeMap::new(),
        //     state_map,
        // }
        todo!()
    }

    fn pop_output_buffer(
        output_buffer: &mut OutputBuffer<'map, T>,
        poll_time: T::Time,
    ) -> Option<(T::Time, T::Output)> {
        let mut entry = output_buffer.first_entry()?; 
        let time = entry.key().raw_time();
        if time > poll_time {
            return None;
        }

        let vec = entry.get_mut();
        let out = vec.pop().unwrap();
        if vec.is_empty() {
            output_buffer.pop_first();
        }
        Some((time, out))
    }

    fn push_output_buffer(
        output_buffer: &mut OutputBuffer<'map, T>,
        time: EngineTime<'map, T::Time>,
        outputs: Vec<T::Output>,
    ) {
        if let Some(_) = output_buffer.insert(time, outputs) {
            // enginetimes are supposed to be unique, so if these are the same something bad has happened.
            unreachable!()
        }
    }

    fn output_buffer_first_time<'a>(
        output_buffer: &'a OutputBuffer<'map, T>
    ) -> Option<&'a EngineTime<'map, T::Time>> {
        let (key, _) = output_buffer.first_key_value()?;
        Some(key)
    }

    fn interpolate(&self, poll_time: T::Time, input_state: T::InputState) -> Option<T::OutputState> {
        let i = self.state_map.last_index_by(poll_time, |x| x.time.raw_time());

        let (item, buffer) = self.state_map.get(i)?;
        let buffer = buffer?;

        if !buffer.is_terminated() {
            return None;
        }

        Some(buffer.transposer_frame.transposer.interpolate(item.time.raw_time(), poll_time, input_state))
    }

    /// this function responds to a rollback event
    ///
    /// returns the time to emit a rollback for, if needed.
    fn handle_input_rollback(
        input_buffer: &mut InputBuffer<T::Time, T::Input>,
        mut state_map: Pin<&mut StateMap<'map, T, N>>,
        output_buffer: &mut OutputBuffer<'map, T>,
        time: T::Time,
    ) -> Option<T::Time> {
        input_buffer.rollback(time);

        let mut rollback_needed = None;
        loop {
            let last_state_map_time = state_map.as_mut().peek().time.raw_time();

            if time > last_state_map_time {
                break;
            }

            match state_map.as_mut().pop() {
                Some(update_item) => {
                    if update_item.events_emitted {
                        rollback_needed = Some(last_state_map_time);

                        // throw away everything at or after the discarded frame.
                        output_buffer.split_off(&update_item.time);
                    }
                }
                None => break
            }
        }

        rollback_needed
    }

    /// this function sets up the gurantee that:
    /// self.input_buffer.first_time() > self.state_map.peek().time
    ///
    /// returns the time to emit a rollback for, if needed.
    fn resolve_state_map_and_input_buffer(
        input_buffer: &mut InputBuffer<T::Time, T::Input>,
        mut state_map: Pin<&mut StateMap<'map, T, N>>,
        output_buffer: &mut OutputBuffer<'map, T>,
    ) -> Option<T::Time> {
        let mut rollback_needed = None;
        let next_input_time = match input_buffer.first_time() {
            Some(t) => t,
            None => return None, // no inputs means we're good here.
        };
        loop {
            let last_state_map_time = state_map.as_mut().peek().time.raw_time();

            if next_input_time > last_state_map_time {
                break;
            }

            match state_map.as_mut().pop() {
                Some(update_item) => {
                    if update_item.events_emitted {
                        rollback_needed = Some(last_state_map_time);

                        // throw away everything at or after the discarded frame.
                        output_buffer.split_off(&update_item.time);
                    }
                    match update_item.data {
                        UpdateItemData::Init(_) => unreachable!(),
                        UpdateItemData::Input(inputs) => {
                            let time = match update_item.time {
                                EngineTime::Input(t) => t,
                                _ => unreachable!(),
                            };
                            input_buffer.extend_front(time, inputs);
                        }
                        UpdateItemData::Schedule => {}
                    }
                }
                None => break
            }
        }

        rollback_needed
    }
}

impl<
    'map,
    T: Transposer + Clone + 'map,
    S: EventStateStream<Time = T::Time, Event = T::Input, State = T::InputState>,
    const N: usize,
    > EventStateStream for TransposerEngine<'map, T, S, N>
    where T::Scheduled: Clone 
{
    type Time = T::Time;
    type Event = T::Output;
    type State = T::OutputState;

    fn poll(
        mut self: Pin<&mut Self>,
        poll_time: Self::Time,
        cx: &mut Context<'_>,
    ) -> EventStatePoll<Self::Time, Self::Event, Self::State> {
        let this = self.as_mut().project();
        let mut input_stream: Pin<&mut S> = this.input_stream;
        let input_buffer: &mut InputBuffer<T::Time, T::Input> = this.input_buffer;
        let output_buffer: &mut OutputBuffer<'map, T> = this.output_buffer;
        let mut state_map: Pin<&mut StateMap<'map, T, N>> = this.state_map;

        enum InputStateEventUpdate<T: Transposer> {
            None,
            Event(T::Time, T::Input),
            Rollback(T::Time),
        }

        let mut input_state_event_update = InputStateEventUpdate::<T>::None;

        'main: loop {
            // resolve events and rollbacks from previous iteration.
            match input_state_event_update {
                InputStateEventUpdate::None => {}
                InputStateEventUpdate::Event(time, input) => {
                    input_buffer.insert_back(time, input);
                    Self::resolve_state_map_and_input_buffer(input_buffer, state_map.as_mut(), output_buffer);
                    input_state_event_update = InputStateEventUpdate::None;
                }
                InputStateEventUpdate::Rollback(time) => {
                    let rollback_time = Self::handle_input_rollback(input_buffer, state_map.as_mut(), output_buffer, time);
                    if let Some(rollback_time) = rollback_time {
                        break 'main EventStatePoll::Rollback(rollback_time);
                    }
                    Self::resolve_state_map_and_input_buffer(input_buffer, state_map.as_mut(), output_buffer);
                    input_state_event_update = InputStateEventUpdate::None;
                }
            }

            let last_buffered_index_before_poll = state_map.as_mut().last_buffered_index_by(poll_time, |x| x.time.raw_time());

            let update_and_buffer = state_map.as_mut().get_pinned_mut(last_buffered_index_before_poll);
            let update_and_buffer = update_and_buffer.unwrap();

            let mut update: Pin<&mut UpdateItem<'map, T>> = update_and_buffer.0;
            let mut buffer: Pin<&mut BufferedItem<'map, T>> = update_and_buffer.1.unwrap();

            // if we have a buffered output before our update, emit that.
            if let Some(output_time) = Self::output_buffer_first_time(output_buffer) {
                if output_time < &update.time {
                    match input_stream.as_mut().poll_events(output_time.raw_time(), cx) {
                        EventStatePoll::Pending => break 'main EventStatePoll::Pending,
                        EventStatePoll::Rollback(time) => {
                            input_state_event_update = InputStateEventUpdate::Rollback(time);
                            continue 'main;
                        }
                        EventStatePoll::Event(time, input) => {
                            input_state_event_update = match T::can_handle(time, &input) {
                                true => InputStateEventUpdate::Event(time, input),
                                false => InputStateEventUpdate::None,
                            };
                            continue 'main;
                        }
                        _ => {}
                    }

                    let (time, input) = Self::pop_output_buffer(output_buffer, poll_time).unwrap();
                    break 'main EventStatePoll::Event(time, input);
                }
            }

            // try to advance the first dependent buffer to complete
            if !buffer.is_terminated() {

                // verify there are no new events or rollbacks before proceeding
                // if we need a state, obtain it and set buffer.input_state
                match buffer.input_state.requested() {
                    true => {
                        let state = match input_stream.as_mut().poll(update.time.raw_time(), cx) {
                            EventStatePoll::Pending => break 'main EventStatePoll::Pending,
                            EventStatePoll::Rollback(time) => {
                                input_state_event_update = InputStateEventUpdate::Rollback(time);
                                continue 'main;
                            }
                            EventStatePoll::Event(time, input) => {
                                input_state_event_update = match T::can_handle(time, &input) {
                                    true => InputStateEventUpdate::Event(time, input),
                                    false => InputStateEventUpdate::None,
                                };
                                continue 'main;
                            }
                            EventStatePoll::Scheduled(time, state) => {
                                // TODO what do i do with the scheduled time here?

                                state
                            }
                            EventStatePoll::Ready(state) => state,
                            EventStatePoll::Done(state) => state,
                        };

                        // pass in our new state.
                        buffer.as_mut().project().input_state.set(state);
                    }
                    false => {
                        match input_stream.as_mut().poll_events(update.time.raw_time(), cx) {
                            EventStatePoll::Pending => break 'main EventStatePoll::Pending,
                            EventStatePoll::Rollback(time) => {
                                input_state_event_update = InputStateEventUpdate::Rollback(time);
                                continue 'main;
                            }
                            EventStatePoll::Event(time, input) => {
                                input_state_event_update = match T::can_handle(time, &input) {
                                    true => InputStateEventUpdate::Event(time, input),
                                    false => InputStateEventUpdate::None,
                                };
                                continue 'main;
                            }
                            EventStatePoll::Scheduled(time, ()) => {
                                // TODO what do i do with the scheduled time here?
                            }
                            EventStatePoll::Ready(()) => {},
                            EventStatePoll::Done(()) => {},
                        }
                    }
                }

                // poll the actual update.
                match buffer.poll(cx) {
                    Poll::Ready(outputs) => {
                        if update.events_emitted || outputs.is_empty() {
                            // we do not want to do anything with events if they have already been emitted.
                            std::mem::drop(outputs);
                            continue 'main;
                        } else {
                            update.events_emitted = true;
                            Self::push_output_buffer(output_buffer, update.time.clone(), outputs);
                            let (time, input) = Self::pop_output_buffer(output_buffer, poll_time).unwrap();
                            break 'main EventStatePoll::Event(time, input);
                        }
                    }
                    Poll::Pending => break 'main EventStatePoll::Pending
                }
            }

            // now buffer is terminated.
            // we need to determine which of the following to do:
            //  - poll input at poll_time, and perform interpolation
            //  - buffer an existing update
            //  - create a new update and buffer it

            let last_index_before_poll = state_map.as_mut().last_index_by(poll_time, |x| x.time.raw_time());
            let last_index = state_map.len() - 1;

            let previously_processed_future_update = last_index != last_buffered_index_before_poll;

        }
    }
}
