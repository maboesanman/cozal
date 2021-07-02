use core::borrow::Borrow;

use super::super::sparse_buffer_stack::SparseBufferStack;

#[test]
fn basic_test() {
    let sparse_buffer_stack: SparseBufferStack<'_, usize, (&usize, usize), 10>;
    sparse_buffer_stack = SparseBufferStack::new(17, |i| (i, 0));
    let mut stack_pin = Box::pin(sparse_buffer_stack);

    let dup = |prev_b: &(&usize, usize), i| (i, prev_b.1 + 2);

    let refurb = |b: &mut (&usize, usize), i| {
        b.0 = i;
        b.1 += 2;
    };

    // the items here are 17, 18, 19, ...
    // the buffered items are 2, 4, 6, 8, ... each paired with a pointer to the corresponding item
    for _ in 0..100 {
        stack_pin.as_mut().push(|x| x + 1);
        let index = stack_pin.borrow().len() - 1;
        assert!(stack_pin.as_mut().buffer(index, dup, refurb).is_ok());
    }

    // buffer an item based on an old checkpoint
    assert!(stack_pin.as_mut().buffer(65, dup, refurb).is_ok());

    // buffer an item which skips cloning
    assert!(stack_pin.as_mut().buffer(66, dup, refurb).is_ok());

    // buffer an item which doesn't have the previous buffer available
    assert!(stack_pin.as_mut().buffer(69, dup, refurb).is_err());

    // buffer a non existent item
    assert!(stack_pin.as_mut().buffer(1000, dup, refurb).is_err());

    // find the last buffered index
    assert_eq!(
        stack_pin.as_mut().last_buffered_index_by(69, |x| *x - 17),
        66
    );
}
