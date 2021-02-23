use std::{collections::BTreeMap, marker::PhantomPinned, mem::MaybeUninit, pin::Pin, task::Context};
use futures::channel::oneshot::Sender;
use pin_project::pin_project;

use crate::core::Transposer;

use super::{engine_time::EngineTime, lazy_state::LazyState, pin_stack::PinStack, transposer_frame::TransposerFrame, transposer_update::TransposerUpdate, update_result::UpdateResult};

#[pin_project]
pub struct StateMap<'map, T: Transposer + 'map, const N: usize> {
    // the order here is very important. state_buffer must outlive its pointers stored in update_stack.
    update_stack: PinStack<UpdateItem<'map, T>>,

    #[pin]
    state_buffer: [StateBufferItem<'map, T>; N],
}

impl<'map, T: Transposer + 'map, const N: usize> StateMap<'map, T, N> {
    pub fn new() -> Self {
        Self {
            update_stack: PinStack::new(),
            state_buffer: array_init::array_init(|_| StateBufferItem::new_zeroed()),
        }
    }

    pub fn rollback(
        self: Pin<&mut Self>,
        rollback_time: T::Time,
    ) {

    }

    pub fn poll(
        self: Pin<&mut Self>,
        poll_time: T::Time,
        input_state: T::InputState,
        input_buffer: &mut BTreeMap<T::Time, Vec<T::Input>>,
        _cx: Context,
    ) -> StateMapPoll<T>{
        let project = self.project();
        let update_stack: &mut PinStack<UpdateItem<T>> = project.update_stack;
        let mut state_buffer: Pin<&mut [StateBufferItem<'_, T>]> = project.state_buffer;

        todo!()
    }

    fn poll_future(
        self: Pin<&mut Self>,
        poll_time: T::Time,
        input_buffer: &mut BTreeMap<T::Time, Vec<T::Input>>,
        _cx: Context,
    ) -> StateMapPoll<T> {
        todo!()
    }

    fn poll_past(
        self: Pin<&mut Self>,
        poll_time: T::Time,
        input_buffer: &mut BTreeMap<T::Time, Vec<T::Input>>,
        _cx: Context,
    ) -> StateMapPoll<T> {
        todo!()
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
    fn get_least_useful_index(buffer: &[StateBufferItem<'map, T>], item: T, new_index: usize, important_indices: &[usize]) -> (usize, usize) {
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

// pointer needs to be the top argument as its target may have pointers into inputs or transposer.
struct UpdateItem<'a, T: Transposer> {
    buffer_index: usize,
    time: EngineTime<'a, T::Time>,
    input_state: LazyState<T::InputState>,
    data: UpdateItemData<T>,
}

enum UpdateItemData<T: Transposer> {
    Init{
        transposer: T,
    },
    Input{
        inputs: Vec<T::Input>,
    },
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

struct CachedState<'a, T: Transposer> {
    input_state: LazyState<T::InputState>,
    transposer_frame: TransposerFrame<'a, T>,
    update_future: CachedStateUpdate<'a, T>,
}

enum CachedStateUpdate<'a, T: Transposer> {
    Working(TransposerUpdate<'a, T>),
    Ready,
}

pub enum StateMapPoll<T: Transposer> {
    Ready{
        state: T::OutputState,
    },
    Outputs(Vec<T::Output>),
    Rollback(T::Time),
    NeedsState{
        time: T::Time,
        sender: Sender<T::InputState>,
    },
    Pending,
}