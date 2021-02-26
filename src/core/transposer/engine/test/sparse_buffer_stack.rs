use crate::core::transposer::engine::sparse_buffer_stack::SparseBufferStack;

#[test]
fn basic_test() {
    let mut sparse_buffer_stack: SparseBufferStack<'_, usize, (&usize, usize), 10>;
    sparse_buffer_stack = SparseBufferStack::new(17, |x| (x, 1));

    for _ in 0..100 {
        sparse_buffer_stack.push(|x| x + 1);
        assert!(sparse_buffer_stack.buffer(sparse_buffer_stack.len() - 1, |s, b| *b = (s, b.1 + *s)).is_ok());
    }
    assert!(sparse_buffer_stack.buffer(65, |s, b| *b = (s, b.1 + *s)).is_ok());
}