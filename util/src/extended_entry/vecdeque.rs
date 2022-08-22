use std::collections::VecDeque;
use std::hint::unreachable_unchecked;

pub fn get_ext_entry<T>(vec_deque: &mut VecDeque<T>, index: usize) -> Option<ExtEntry<'_, T>> {
    if index < vec_deque.len() {
        Some(ExtEntry {
            vec_deque,
            index,
        })
    } else {
        None
    }
}

#[derive(Debug)]
pub struct ExtEntry<'a, T> {
    vec_deque: &'a mut VecDeque<T>,
    index:     usize,
}

impl<'a, T> ExtEntry<'a, T> {
    pub fn get_index(&self) -> usize {
        self.index
    }

    pub fn get_value(&self) -> &T {
        match self.vec_deque.get(self.index) {
            Some(t) => t,
            None => unsafe { unreachable_unchecked() },
        }
    }

    pub fn get_value_mut(&mut self) -> &mut T {
        match self.vec_deque.get_mut(self.index) {
            Some(t) => t,
            None => unsafe { unreachable_unchecked() },
        }
    }

    pub fn into_value_mut(self) -> &'a mut T {
        match self.vec_deque.get_mut(self.index) {
            Some(t) => t,
            None => unsafe { unreachable_unchecked() },
        }
    }

    pub fn into_collection_mut(self) -> &'a mut VecDeque<T> {
        self.vec_deque
    }
}
