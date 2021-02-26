use std::{cmp::{self, Ordering}, collections::BTreeMap, marker::PhantomPinned, mem::MaybeUninit, pin::Pin, task::Context};
use cmp::min;
use pin_project::pin_project;
use futures::Future;

use crate::core::Transposer;

use super::{engine_time::EngineTime, input_buffer::InputBuffer, lazy_state::LazyState, pin_stack::PinStack, transposer_frame::TransposerFrame, transposer_update::TransposerUpdate, update_result::UpdateResult};

#[pin_project]
pub struct StateMap<'map, T: Transposer + Clone + 'map, const N: usize>
where T::Scheduled: Clone {
    // the order here is very important. state_buffer must outlive its pointers stored in update_stack.
    update_stack: PinStack<UpdateItem<'map, T>>,

    #[pin]
    state_buffer: [StateBufferItem<'map, T>; N],
}

impl<'map, T: Transposer + Clone + 'map, const N: usize> StateMap<'map, T, N>
where T::Scheduled: Clone {
    pub fn new() -> Self {
        Self {
            update_stack: PinStack::new(),
            state_buffer: array_init::array_init(|_| StateBufferItem::new_zeroed()),
        }
    }

    #[allow(unused)]
    pub fn rollback(
        self: Pin<&mut Self>,
        rollback_time: T::Time,
    ) -> Option<T::Time> {
        todo!()
    }

    #[allow(unused)]
    pub fn poll(
        mut self: Pin<&mut Self>,
        poll_time: T::Time,
        input_state: T::InputState,
        input_buffer: &mut InputBuffer<T::Time, T::Input>,
        cx: &mut Context,
    ) -> StateMapPoll<T> {
        match self.as_mut().poll_events(poll_time, input_buffer, cx) {
            StateMapEventPoll::Ready => {
                let output_state = self.interpolate(poll_time, input_state);
                StateMapPoll::Ready(output_state)
            },
            StateMapEventPoll::Outputs(o) => StateMapPoll::Outputs(o),
            StateMapEventPoll::Rollback(t) => StateMapPoll::Rollback(t),
            StateMapEventPoll::NeedsState(t) => StateMapPoll::NeedsState(t),
            StateMapEventPoll::Pending => StateMapPoll::Pending,
        }
    }

    #[allow(unused)]
    pub fn poll_events(
        mut self: Pin<&mut Self>,
        poll_time: T::Time,
        input_buffer: &mut InputBuffer<T::Time, T::Input>,
        cx: &mut Context,
    ) -> StateMapEventPoll<T> {
        if let Some(t) = self.as_mut().rollback_prep(poll_time, input_buffer) {
            return StateMapEventPoll::Rollback(t);
        }
        let this = self.project();
        let update_stack: &mut PinStack<UpdateItem<'map, T>> = this.update_stack;
        let mut state_buffer: Pin<&mut [StateBufferItem<'map, T>]> = this.state_buffer;

        // find the most recent buffered state (and index)
        let range = update_stack.range_by(..=poll_time, |update_item| update_item.time.raw_time());
        let (last_buffered_update_index, _) = range.rev().find(|(update_index, update_item)| {

            // this update has a buffered state
            state_buffer.get(update_item.buffer_index).unwrap().update_index == *update_index
        }).unwrap();

        loop {
            let last_buffered_update = update_stack.get_mut(last_buffered_update_index).unwrap();
            // SAFEFY: going from a pinned array to a pinned reference into it. this should be fine.
            let state_buffer = unsafe { state_buffer.as_mut().get_unchecked_mut() };

            let state_buffer_item = state_buffer.get_mut(last_buffered_update.buffer_index).unwrap();
            let cached_state = unsafe { state_buffer_item.cached_state.assume_init_mut() };
            let cached_state = unsafe { Pin::new_unchecked(cached_state) };

            let mut cached_state = cached_state.project();
            let update_future = cached_state.update_future.as_mut().project();
            if let CachedStateUpdateProject::Working(transposer_update) = update_future {
                let transposer_update: Pin<&mut TransposerUpdate<'_, T>> = transposer_update;

                match transposer_update.poll(cx) {
                    std::task::Poll::Ready(update_result) => {
                        let update_future = unsafe { cached_state.update_future.get_unchecked_mut()};
                        *update_future = CachedStateUpdate::Ready;
                        if last_buffered_update.events_emitted && !update_result.outputs.is_empty() {
                            *last_buffered_update.project().events_emitted = true;
                            break StateMapEventPoll::Outputs(update_result.outputs)
                        }
                    }
                    std::task::Poll::Pending => match cached_state.input_state {
                        LazyState::Requested => break StateMapEventPoll::NeedsState(last_buffered_update.time.raw_time()),
                        LazyState::Ready(_) => break StateMapEventPoll::Pending,
                        LazyState::Pending => break StateMapEventPoll::Pending,
                    }
                }
            }
            // transposer frame depends on items before it in the stack, so it would have to be dropped before its dependencies.
            let transposer_frame: &mut TransposerFrame<'map, T> = cached_state.transposer_frame;
            let transposer_frame = transposer_frame as *mut TransposerFrame<'map, T>;
            let transposer_frame = unsafe { transposer_frame.as_mut().unwrap()};

            // here our update is done, so we can use transposer_frame.
            if last_buffered_update_index == update_stack.len() - 1 {
                let next_input_time = input_buffer.first_time();
                let next_schedule_time = transposer_frame.get_next_schedule_time();

                let (time, data) = match (next_input_time, next_schedule_time) {
                    (None, None) => break StateMapEventPoll::Ready,
                    (Some(t), None) => (EngineTime::new_input(t), UpdateItemData::Input(input_buffer.pop_first().unwrap().1)),
                    (None, Some(t)) => (EngineTime::from(t.clone()), UpdateItemData::Schedule),
                    (Some(i), Some(s)) => match i.cmp(&s.time) {
                        Ordering::Less => (EngineTime::new_input(i), UpdateItemData::Input(input_buffer.pop_first().unwrap().1)),
                        Ordering::Equal => (EngineTime::from(s.clone()), UpdateItemData::Schedule),
                        Ordering::Greater => (EngineTime::from(s.clone()), UpdateItemData::Schedule),
                    }
                };

                update_stack.push(UpdateItem {
                    buffer_index: usize::MAX,
                    time,
                    data,
                    events_emitted: false,
                });
            }

            let working_update_stack: &'map _ = update_stack.get(last_buffered_update_index + 1).unwrap();
            if working_update_stack.time.raw_time() > poll_time {
                break StateMapEventPoll::Ready
            }

            // reinsert next frame into state buffer
            let (buffer_index_to_replace, update_index_to_replace) = Self::get_least_useful_index(state_buffer, &[update_stack.len() - 1]);
            
            // get and update our buffered state.
            let buffered_state = state_buffer.get_mut(buffer_index_to_replace).unwrap();
            buffered_state.update_index = last_buffered_update_index + 1;

            if update_index_to_replace == last_buffered_update_index {
                // cached_state.transposer_frame is already the correct one; no need to clone.
            } else {
                Self::drop_buffered();
                
                let previous_buffered_index = update_stack.get(last_buffered_update_index).unwrap().buffer_index;
                let previous_state_buffer = state_buffer.get_mut(previous_buffered_index).unwrap();
                let previous_cached_state = unsafe { previous_state_buffer.cached_state.assume_init_ref() };
                let previous_transposer_frame = previous_cached_state.transposer_frame;
                cached_state.transposer_frame = previous_transposer_frame.clone();
            }

            let cached_state = unsafe { buffered_state.cached_state.assume_init_mut() };

            if let CachedStateUpdate::Working(_) = cached_state.update_future {
                panic!("tried to work with an in progress transposer_frame")
            }
            cached_state.transposer_frame.internal.advance_time(&working_update_stack.time);
            let transposer_update = TransposerUpdate::new();
            cached_state.update_future = CachedStateUpdate::Working(transposer_update);
            let cached_state = unsafe { Pin::new_unchecked(cached_state) };
            cached_state.input_state = LazyState::Pending;

            let cached_state = cached_state.project();
            let transposer_frame: &mut TransposerFrame<'_, T> = cached_state.transposer_frame;
            let update_future: Pin<&mut CachedStateUpdate<'_, T>> = cached_state.update_future;
            let transposer_update: Pin<&mut TransposerUpdate<'_, T>> = match update_future.project() {
                CachedStateUpdateProject::Working(x) => x,
                CachedStateUpdateProject::Ready => unreachable!(),
            };
            match working_update_stack.data {
                UpdateItemData::Init(transposer) => transposer_update.start_init(transposer_frame, cached_state.input_state),
                UpdateItemData::Input(inputs) => transposer_update.start_input(transposer_frame, cached_state.input_state),
                UpdateItemData::Schedule => transposer_update.start_schedule(transposer_frame,cached_state.input_state),
            }
        }
    }

    fn rollback_prep(
        self: Pin<&mut Self>,
        poll_time: T::Time,
        input_buffer: &mut InputBuffer<T::Time, T::Input>,
    ) -> Option<T::Time> {
        let this = self.project();
        let update_stack: &mut PinStack<UpdateItem<'map, T>> = this.update_stack;
        let mut state_buffer: Pin<&mut [StateBufferItem<'map, T>]> = this.state_buffer;

        let next_input_time = input_buffer.first_time();

        let inputs_handled = next_input_time.map_or(true, |next_input_time| poll_time < next_input_time);

        // rollback internally if we need to
        if inputs_handled {
            None
        } else {
            let mut needs_rollback: Option<T::Time> = None;
            loop {
                let current_time = update_stack.peek().unwrap().time.raw_time();
                if next_input_time.unwrap() <= current_time {
                    let UpdateItem {
                        buffer_index,
                        time,
                        data,
                        events_emitted,
                    } = update_stack.pop().unwrap();
                    let update_index = update_stack.len();

                    if let UpdateItemData::Input(inputs) = data {
                        input_buffer.extend_front(time.raw_time(), inputs);
                    }

                    if events_emitted {
                        needs_rollback = match needs_rollback {
                            Some(t) => Some(cmp::min(t, time.raw_time())),
                            None => Some(time.raw_time())
                        };
                    }

                    Self::drop_buffered(state_buffer.as_mut(), buffer_index, update_index)
                } else {
                    break;
                }
            }

            needs_rollback
        }
    }

    fn interpolate(&self, poll_time: T::Time, input_state: T::InputState) -> T::OutputState {
        let range = self.update_stack.range_by(..=poll_time, |update_item| update_item.time.raw_time());
        let (_, latest_update) = range.last().unwrap();
        let state_buffer_item = self.state_buffer.get(latest_update.buffer_index).unwrap();

        if state_buffer_item.update_index != self.update_stack.len() - 1 {
            panic!("uh oh");
        }

        let cached_state = unsafe { state_buffer_item.cached_state.assume_init_ref() };
        if let CachedStateUpdate::Working(_) = cached_state.update_future {
            panic!("uh oh");
        }

        cached_state.transposer_frame.transposer.interpolate(latest_update.time.raw_time(), poll_time, input_state)
    }

    fn drop_buffered(buffer: Pin<&mut [StateBufferItem<'map, T>]>, buffer_index: usize, expected_update_index: usize) {
        if expected_update_index == usize::MAX {
            panic!("tried to drop an empty StateBufferItem")
        }
        
        let buffer = unsafe { buffer.get_unchecked_mut() };
        if let Some(item) = buffer.get_mut(buffer_index) {
            if expected_update_index == item.update_index {
                unsafe { item.assume_init_drop() }
            }
        }
    }

    // get the buffer index and input_index of the next item to delete.
    fn get_least_useful_index(buffer: &[StateBufferItem<'map, T>], important_indices: &[usize]) -> (usize, usize) {
        let iter = buffer.iter().enumerate();
        let (buffer_index, item) = iter.min_by_key(|&(_, x)| {
            let utility = important_indices.iter().map(
                |&index| (index as isize - x.update_index as isize
            ).leading_zeros()).max().unwrap_or(0);
            let utility = utility + x.update_index.trailing_zeros();
            utility
        }).unwrap();

        (buffer_index, item.update_index)
    }
}

#[pin_project]
// pointer needs to be the top argument as its target may have pointers into inputs or transposer.
struct UpdateItem<'a, T: Transposer> {
    buffer_index: usize,
    #[pin]
    time: EngineTime<'a, T::Time>,
    #[pin]
    data: UpdateItemData<T>,
    events_emitted: bool,
}

enum UpdateItemData<T: Transposer> {
    Init(Box<T>),
    Input(Vec<T::Input>),
    Schedule
}

struct StateBufferItem<'tr, T: Transposer + 'tr> {
    update_index: usize,
    cached_state: MaybeUninit<CachedState<'tr, T>>,
}

impl<'tr, T: Transposer + 'tr> StateBufferItem<'tr, T> {
    pub fn new_zeroed() -> Self {
        Self {
            update_index: usize::MAX,
            cached_state: MaybeUninit::uninit()
        }
    }

    fn new(input_index: usize, item: CachedState<'tr, T>) -> Self {
        Self {
            update_index: input_index,
            cached_state: MaybeUninit::new(item),
        }
    }

    unsafe fn assume_init_drop(&mut self) {
        self.update_index = usize::MAX;
        self.cached_state.assume_init_drop()
    }
}

#[pin_project(project=CachedStateProject)]
struct CachedState<'a, T: Transposer> {
    // this is first because it has references to other fields.
    // don't want this to dangle anything
    #[pin]
    update_future: CachedStateUpdate<'a, T>,

    transposer_frame: TransposerFrame<'a, T>,

    input_state: LazyState<T::InputState>,

    // update_future has references into both transposer_frame and input_state
    _marker: PhantomPinned,
}

#[pin_project(project=CachedStateUpdateProject)]
enum CachedStateUpdate<'a, T: Transposer> {
    Working(#[pin] TransposerUpdate<'a, T>),
    Ready,
}

pub enum StateMapPoll<T: Transposer> {
    Ready(T::OutputState),
    Outputs(Vec<T::Output>),
    Rollback(T::Time),
    NeedsState(T::Time),
    Pending,
}

pub enum StateMapEventPoll<T: Transposer> {
    Ready,
    Outputs(Vec<T::Output>),
    Rollback(T::Time),
    NeedsState(T::Time),
    Pending,
}