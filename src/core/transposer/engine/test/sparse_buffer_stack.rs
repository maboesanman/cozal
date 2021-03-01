use std::{borrow::Borrow, pin::Pin};

use crate::core::transposer::engine::sparse_buffer_stack::SparseBufferStack;

#[test]
fn basic_test() {
    let mut sparse_buffer_stack: SparseBufferStack<'_, usize, (&usize, usize), 10>;
    sparse_buffer_stack = SparseBufferStack::new(17, |x| (x, 1), |_, _| {});
    let stack_ref = &mut sparse_buffer_stack;
    let mut stack_pin = unsafe { Pin::new_unchecked(stack_ref) };

    for _ in 0..100 {
        stack_pin.as_mut().push(|x| x + 1);
        let index = stack_pin.borrow().len() - 1;
        assert!(stack_pin.as_mut().buffer(index, |b| b.clone(), |s, mut b| *b = (s, b.1 + *s)).is_ok());
    }

    // buffer an item based on an old checkpoint
    assert!(stack_pin.as_mut().buffer(65, |b| b.clone(), |s, mut b| *b = (s, b.1 + *s)).is_ok());

    // buffer an item which skips cloning
    assert!(stack_pin.as_mut().buffer(66, |b| b.clone(), |s, mut b| *b = (s, b.1 + *s)).is_ok());

    // buffer an item which doesn't have the previous buffer available
    assert!(stack_pin.as_mut().buffer(69, |b| b.clone(), |s, mut b| *b = (s, b.1 + *s)).is_err());

    // buffer a non existent item
    assert!(stack_pin.as_mut().buffer(1000, |b| b.clone(), |s, mut b| *b = (s, b.1 + *s)).is_err());

    // find the last buffered index
    assert_eq!(stack_pin.as_mut().last_buffered_index_by(69, |x| *x - 17), 66);
}