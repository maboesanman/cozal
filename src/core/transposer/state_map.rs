use std::{collections::BTreeMap, mem::MaybeUninit, pin::Pin, sync::Arc, task::Context};
use futures::channel::oneshot::Sender;
use pin_project::pin_project;

use super::{Transposer, dynamic_index_buffer::{BufferPointer, DynamicBuffer}, engine_context::LazyState, engine_time::EngineTime, pin_stack::PinStack, transposer_frame::TransposerFrame, update_result::UpdateResult, wrapped_future::WrappedFuture};

#[pin_project]
pub struct StateMap<T: Transposer, const N: usize> {
    // the order here is very important. state_buffer must outlive its pointers stored in update_stack.
    update_stack: PinStack<UpdateItem<T>>,
    state_buffer: [MaybeUninit<CachedState<T>>; N],
}

impl<T: Transposer, const N: usize> StateMap<T, N> {
    pub fn new() -> Self {
        Self {
            update_stack: PinStack::new(),
            state_buffer: DynamicBuffer::new(),
        }
    }

    pub fn poll(
        self: Pin<&mut Self>,
        time: T::Time,
        input_buffer: &mut BTreeMap<T::Time, Vec<T::Input>>,
        cx: Context,
    ) -> StateMapPoll<T>{
        todo!()
    }
}

// pointer needs to be the top argument as its target may have pointers into inputs or transposer.
enum UpdateItem<'a, T: Transposer> {
    Init{
        pointer: Pin<&'a mut CachedState<'a, T>>,
        transposer: T,
        time: Arc<EngineTime<T::Time>>,
        input_state: LazyState<T::InputState>,
    },
    Input{
        pointer: Pin<&'a mut CachedState<'a, T>>,
        time: Arc<EngineTime<T::Time>>,
        inputs: Vec<T::Input>,
        input_state: LazyState<T::InputState>,
    },
    Schedule{
        pointer: Pin<&'a mut CachedState<'a, T>>,
        time: Arc<EngineTime<T::Time>>,
        input_state: LazyState<T::InputState>,
    }
}

struct CachedState<'a, T: Transposer> {
    input_state: LazyState<T::InputState>,
    transposer_frame: TransposerFrame<T>,
    update_future: CachedStateUpdate<'a, T>,
}

enum CachedStateUpdate<'a, T: Transposer> {
    Working(WrappedFuture<'a, T>),
    Ready(UpdateResult<T>),
}

pub enum StateMapPoll<T: Transposer> {
    Ready(T::OutputState),
    NeedsState{
        time: T::Time,
        sender: Sender<T::InputState>
    },
    Pending,
}