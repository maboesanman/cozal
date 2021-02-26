use crate::core::transposer::engine::sparse_buffer_stack::SparseBufferStack;

#[test]
fn basic_test() {
    let mut sparse_buffer_stack: SparseBufferStack<'_, usize, (&usize, usize), 10>;
    sparse_buffer_stack = SparseBufferStack::new(17, |x| (x, 1));

    for _ in 0..100 {
        sparse_buffer_stack.push(|x| x + 1);
        assert!(sparse_buffer_stack.buffer(sparse_buffer_stack.len() - 1, |s, b| *b = (s, b.1 + *s)).is_ok());
    }

    // buffer an item based on an old checkpoint
    assert!(sparse_buffer_stack.buffer(65, |s, b| *b = (s, b.1 + *s)).is_ok());

    // buffer an item which skips cloning
    assert!(sparse_buffer_stack.buffer(66, |s, b| *b = (s, b.1 + *s)).is_ok());

    // buffer an item which doesn't have the previous buffer available
    assert!(sparse_buffer_stack.buffer(69, |s, b| *b = (s, b.1 + *s)).is_err());

    // buffer a non existent item
    assert!(sparse_buffer_stack.buffer(1000, |s, b| *b = (s, b.1 + *s)).is_err());
}