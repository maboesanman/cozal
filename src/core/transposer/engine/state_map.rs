use std::{collections::BTreeMap, mem::MaybeUninit, pin::Pin, sync::Arc, task::Context};
use futures::channel::oneshot::Sender;
use pin_project::pin_project;

use crate::core::Transposer;

use super::{engine_context::LazyState, engine_time::EngineTime, pin_stack::PinStack, transposer_frame::TransposerFrame, update_result::UpdateResult, wrapped_future::WrappedFuture};

#[pin_project]
pub struct StateMap<'tr, T: Transposer + 'tr, const N: usize> {
    // the order here is very important. state_buffer must outlive its pointers stored in update_stack.
    update_stack: PinStack<UpdateItem<'tr, T>>,

    // THIS IS STATIC FOR SELF REFERENTIAL REASONS
    // THIS SHOULD BE THOUGHT OF AS A PTR
    state_buffer: [StateBufferItem<'tr, T>; N],
}

impl<'map, T: Transposer + 'map, const N: usize> StateMap<'map, T, N> {
    pub fn new() -> Self {
        Self {
            update_stack: PinStack::new(),
            state_buffer: array_init::array_init(|_| StateBufferItem::new_zeroed()),
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
enum UpdateItem<'tr, T: Transposer + 'tr> {
    Init{
        pointer: Pin<&'tr mut CachedState<'tr, T>>,
        transposer: T,
        time: Arc<EngineTime<T::Time>>,
        input_state: LazyState<T::InputState>,
    },
    Input{
        pointer: Pin<&'tr mut CachedState<'tr, T>>,
        time: Arc<EngineTime<T::Time>>,
        inputs: Vec<T::Input>,
        input_state: LazyState<T::InputState>,
    },
    Schedule{
        pointer: Pin<&'tr mut CachedState<'tr, T>>,
        time: Arc<EngineTime<T::Time>>,
        input_state: LazyState<T::InputState>,
    }
}

struct StateBufferItem<'tr, T: Transposer + 'tr> {
    input_index: usize,
    cached_state: MaybeUninit<CachedState<'tr, T>>,
}

impl<'tr, T: Transposer + 'tr> StateBufferItem<'tr, T> {
    pub fn new_zeroed() -> Self {
        Self {
            input_index: usize::MAX,
            cached_state: MaybeUninit::uninit()
        }
    }

    fn new(input_index: usize, item: CachedState<'tr, T>) -> Self {
        Self {
            input_index,
            cached_state: MaybeUninit::new(item),
        }
    }

    unsafe fn assume_init_drop(&mut self) {
        self.input_index = usize::MAX;
        self.cached_state.assume_init_drop()
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
    Ready{
        state: T::OutputState,

        // this is only returned if we have never emitted these outputs before.
        new_outputs: Vec<T::Output>,
    },
    NeedsState{
        time: T::Time,
        sender: Sender<T::InputState>,
    },
    Pending,
}