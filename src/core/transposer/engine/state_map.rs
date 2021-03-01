use std::{cmp::{self, Ordering}, marker::PhantomPinned, pin::Pin, task::{Context, Poll}};
use pin_project::pin_project;
use futures::Future;

use crate::core::Transposer;

use super::{engine_time::EngineTime, input_buffer::InputBuffer, lazy_state::LazyState, sparse_buffer_stack::SparseBufferStack, transposer_frame::TransposerFrame, transposer_update::TransposerUpdate};

#[pin_project]
pub struct StateMap<'map, T: Transposer + Clone + 'map, const N: usize>
where T::Scheduled: Clone {
    // the order here is very important. state_buffer must outlive its pointers stored in update_stack.
    #[pin]
    inner: SparseBufferStack<'map, UpdateItem<'map, T>, BufferedItem<'map, T>, N>
}

impl<'map, T: Transposer + Clone + 'map, const N: usize> StateMap<'map, T, N>
where T::Scheduled: Clone {
    #[allow(unused)]
    pub fn new(transposer: T) -> Self {
        let update_item = UpdateItem {
            time: EngineTime::Init,
            data: UpdateItemData::Init(Box::new(transposer)),
            events_emitted: false,
        };
        Self {
            inner: SparseBufferStack::new(
                update_item,
                |update_item_ref| {
                    let transposer: T = if let UpdateItemData::Init(data) = &update_item_ref.data {
                        data.as_ref().clone()
                    } else {
                        unreachable!()
                    };
                    let current_time = &update_item_ref.time;
                    BufferedItem {
                        update_future: BufferedUpdate::Working(TransposerUpdate::new()),
                        transposer_frame: TransposerFrame::new(transposer, &current_time),
                        input_state: LazyState::Pending,
            
                        _marker: PhantomPinned
                    }
                },
                |_, buffer_item_ref| {
                    let this = buffer_item_ref.project();
                    let update_future: Pin<&mut BufferedUpdate<'map, T>> = this.update_future;

                    let transposer_frame: &mut TransposerFrame<'map, T> = this.transposer_frame;
                    let input_state: &mut LazyState<T::InputState> = this.input_state;

                    let transposer_frame: *mut TransposerFrame<'map, T> = transposer_frame;
                    let input_state: *mut LazyState<T::InputState> = input_state;

                    // SAFETY: these references are self references, and won't outlive their targets.
                    let transposer_frame: &'map mut TransposerFrame<'map, T> = unsafe { transposer_frame.as_mut() }.unwrap();
                    let input_state: &'map mut LazyState<T::InputState> = unsafe { input_state.as_mut() }.unwrap();

                    if let BufferedUpdateProject::Working(transposer_update) = update_future.project() {
                        let transposer_update: Pin<&mut TransposerUpdate<'map, T>> = transposer_update;
                        transposer_update.start_init(transposer_frame, input_state)
                    } else {
                        unreachable!();
                    }
                }
            )
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
                let output_state = self.interpolate(poll_time, input_state).unwrap();
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
        let mut this = self.project();
        let mut inner: Pin<&mut SparseBufferStack<'map, UpdateItem<'map, T>, BufferedItem<'map, T>, N>>;
        inner = this.inner;

        // // find the most recent buffered state (and index)
        loop {
            let last_buffered_update_index = inner.as_mut().last_buffered_index_by(poll_time, |x| x.time.raw_time());
            let is_current = last_buffered_update_index == inner.as_mut().len() - 1;
            let (update_item, buffered_item) = inner.as_mut().get_pinned_mut(last_buffered_update_index).unwrap();
            let mut buffered_item = buffered_item.unwrap().project();
            let mut update_future = buffered_item.update_future.as_mut().project();

            // if we're still working, poll the update until we're stuck or ready.
            if let BufferedUpdateProject::Working(transposer_update) = update_future {
                let transposer_update: Pin<&mut TransposerUpdate<'map, T>> = transposer_update;
                match transposer_update.poll(cx) {
                    Poll::Ready(update_result) => {
                        update_future = BufferedUpdateProject::Ready;

                        // if there are any output events we need to emit them.
                        // note that when reconstructing a previous state, events_emitted is still true
                        // from the original call. it is never set to false.
                        if update_item.events_emitted && !update_result.outputs.is_empty() {
                            update_item.get_mut().events_emitted = true;
                            break StateMapEventPoll::Outputs(update_result.outputs)
                        }
                    }
                    Poll::Pending => match buffered_item.input_state {
                        LazyState::Requested => break StateMapEventPoll::NeedsState(update_item.time.raw_time()),
                        LazyState::Ready(_) => break StateMapEventPoll::Pending,
                        LazyState::Pending => break StateMapEventPoll::Pending,
                    }
                }
            }

            let transposer_frame: &mut TransposerFrame<'map, T> = buffered_item.transposer_frame;

            if is_current {
                let next_input_time = input_buffer.first_time();
                let next_schedule_time = transposer_frame.get_next_schedule_time();

                let (time, data): (EngineTime<'map, T::Time>, UpdateItemData<T>) = match (next_input_time, next_schedule_time) {
                    (None, None) => break StateMapEventPoll::Ready,
                    (Some(t), None) => (EngineTime::new_input(t), UpdateItemData::Input(input_buffer.pop_first().unwrap().1.into_boxed_slice())),
                    (None, Some(t)) => (EngineTime::from(t.clone()), UpdateItemData::Schedule),
                    (Some(i), Some(s)) => match i.cmp(&s.time) {
                        Ordering::Less => (EngineTime::new_input(i), UpdateItemData::Input(input_buffer.pop_first().unwrap().1.into_boxed_slice())),
                        Ordering::Equal => (EngineTime::from(s.clone()), UpdateItemData::Schedule),
                        Ordering::Greater => (EngineTime::from(s.clone()), UpdateItemData::Schedule),
                    }
                };

                inner.as_mut().push(|_| UpdateItem {
                    time,
                    data,
                    events_emitted: false,
                });
            }

            let next_frame_time = inner.as_mut().get(last_buffered_update_index + 1).unwrap().0.time.raw_time();

            if next_frame_time > poll_time {
                break StateMapEventPoll::Ready;
            }

            // (re)insert the next buffer into the 
            inner.as_mut().buffer(
                last_buffered_update_index + 1,
                |last_buffered_ref: &BufferedItem<T>| {
                    let update_future = BufferedUpdate::Working(TransposerUpdate::new());
                    let transposer_frame = last_buffered_ref.transposer_frame.clone();
                    let input_state = LazyState::Pending;

                    BufferedItem {
                        update_future,
                        transposer_frame,
                        input_state,
                        _marker: PhantomPinned,
                    }
                },
                |update_ref: &UpdateItem<'map, T>, buffer_ref: Pin<&mut BufferedItem<'map, T>>| {
                    let buffer_ref = buffer_ref.project();
                    let update_future: Pin<&mut BufferedUpdate<'map, T>> = buffer_ref.update_future;
                    let transposer_update: Pin<&mut TransposerUpdate<'map, T>>;
                    transposer_update = if let BufferedUpdateProject::Working(transposer_update) = update_future.project() {
                        transposer_update
                    } else {
                        unreachable!()
                    };

                    let transposer_frame: &mut TransposerFrame<T> = buffer_ref.transposer_frame;
                    let input_state: &mut LazyState<T::InputState> = buffer_ref.input_state;

                    let transposer_frame: *mut TransposerFrame<'map, T> = transposer_frame;
                    let input_state: *mut LazyState<T::InputState> = input_state;

                    // SAFETY: these references are self references, and won't outlive their targets.
                    let transposer_frame: &'map mut TransposerFrame<'map, T> = unsafe { transposer_frame.as_mut() }.unwrap();
                    let input_state: &'map mut LazyState<T::InputState> = unsafe { input_state.as_mut() }.unwrap();

                    match (&update_ref.time, &update_ref.data) {
                        (EngineTime::Input(time), UpdateItemData::Input(inputs)) => {
                            transposer_update.start_input(
                                transposer_frame,
                                input_state,
                                *time,
                                inputs
                            );
                        }
                        (EngineTime::Schedule(time_schedule), UpdateItemData::Schedule) => {
                            transposer_update.start_schedule(
                                transposer_frame,
                                input_state
                            );
                        }
                        _ => {
                            // everything else is either init (which can't be here)
                            // or the engine time and update item data don't match.
                            unreachable!()
                        }
                    }
                },
            );
        }
    }

    fn rollback_prep(
        self: Pin<&mut Self>,
        poll_time: T::Time,
        input_buffer: &mut InputBuffer<T::Time, T::Input>,
    ) -> Option<T::Time> {
        let mut this = self.project();
        let inner: &mut Pin<&mut SparseBufferStack<'map, UpdateItem<'map, T>, BufferedItem<'map, T>, N>>;
        inner = &mut this.inner;

        let next_input_time = input_buffer.first_time();

        let inputs_handled = next_input_time.map_or(true, |next_input_time| poll_time < next_input_time);

        // rollback internally if we need to
        if inputs_handled {
            None
        } else {
            let mut needs_rollback: Option<T::Time> = None;
            loop {
                let current_time = inner.peek().time.raw_time();
                if next_input_time.unwrap() <= current_time {
                    // todo this unwrap is sketchy
                    let UpdateItem {
                        time,
                        data,
                        events_emitted,
                    } = inner.as_mut().pop().unwrap();

                    if let UpdateItemData::Input(inputs) = data {
                        input_buffer.extend_front(time.raw_time(), inputs);
                    }

                    if events_emitted {
                        needs_rollback = match needs_rollback {
                            Some(t) => Some(cmp::min(t, time.raw_time())),
                            None => Some(time.raw_time())
                        };
                    }
                } else {
                    break;
                }
            }

            needs_rollback
        }
    }

    fn interpolate(&self, poll_time: T::Time, input_state: T::InputState) -> Option<T::OutputState> {
        let i = self.inner.last_index_by(poll_time, |x| x.time.raw_time());

        let (item, buffer) = self.inner.get(i)?;
        let buffer = buffer?;

        if !buffer.update_future.is_ready() {
            return None;
        }

        Some(buffer.transposer_frame.transposer.interpolate(item.time.raw_time(), poll_time, input_state))
    }
}

// pointer needs to be the top argument as its target may have pointers into inputs or transposer.
#[pin_project]
struct UpdateItem<'a, T: Transposer> {
    #[pin]
    time: EngineTime<'a, T::Time>,
    data: UpdateItemData<T>,
    events_emitted: bool,
}

enum UpdateItemData<T: Transposer> {
    Init(Box<T>),
    Input(Box<[T::Input]>),
    Schedule
}

#[pin_project(project=BufferedItemProject)]
struct BufferedItem<'a, T: Transposer> {
    // this is first because it has references to other fields.
    // don't want this to dangle anything
    #[pin]
    update_future: BufferedUpdate<'a, T>,

    transposer_frame: TransposerFrame<'a, T>,

    input_state: LazyState<T::InputState>,

    // update_future has references into both transposer_frame and input_state
    _marker: PhantomPinned,
}

#[pin_project(project=BufferedUpdateProject)]
enum BufferedUpdate<'a, T: Transposer> {
    Working(#[pin] TransposerUpdate<'a, T>),

    // this is constructed through a pin, so don't worry about unused.
    #[allow(unused)]
    Ready,
}

impl<'a, T: Transposer> BufferedUpdate<'a, T> {
    pub fn is_ready(&self) -> bool {
        match self {
            BufferedUpdate::Ready => true,
            _ => false,
        }
    }
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