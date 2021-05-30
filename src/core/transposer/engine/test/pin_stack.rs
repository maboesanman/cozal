use crate::core::transposer::engine::pin_stack::PinStack;

#[test]
fn basic_test() {
    let mut pin_stack = PinStack::new();

    pin_stack.reserve(0);

    assert_eq!(pin_stack.peek(), None);
    for i in 0..100 {
        pin_stack.push(i);
        assert_eq!(*pin_stack.peek().unwrap(), i);
        assert_eq!(*pin_stack.get(i).unwrap(), i);
        assert_eq!(pin_stack.len(), i + 1);
    }

    for i in 0..100 {
        assert_eq!(pin_stack.get(i), Some(&i));
    }

    for i in (0..100).rev() {
        assert_eq!(pin_stack.pop().unwrap(), i);
    }

    assert_eq!(pin_stack.pop(), None);
    assert_eq!(pin_stack.capacity(), 128);
}

#[test]
fn range_by() {
    let mut pin_stack = PinStack::new();

    for i in 0..100 {
        pin_stack.push(i);
    }

    let mut range = pin_stack.range_by(15..45, |x| *x);
    for i in 15..30 {
        assert_eq!(i, *range.next().unwrap().1);
    }

    for i in (30..45).rev() {
        assert_eq!(i, *range.next_back().unwrap().1);
    }
    assert_eq!(range.next(), None);
}

#[test]
fn no_move() {
    // vec changes the address
    let mut vec = Vec::new();
    let mut pin_stack = PinStack::new();

    for i in 0..69 {
        vec.push(i);
        pin_stack.push(i);
    }

    let element_ref = vec.get(17).unwrap();
    let vec_address_before = element_ref as *const i32 as usize;
    let element_ref = pin_stack.get(17).unwrap();
    let pin_stack_address_before = element_ref as *const i32 as usize;

    for i in 0..420 {
        vec.push(i);
        pin_stack.push(i);
    }

    let element_ref = vec.get(17).unwrap();
    let vec_address_after = element_ref as *const i32 as usize;
    let element_ref = pin_stack.get(17).unwrap();
    let pin_stack_address_after = element_ref as *const i32 as usize;

    // vec doesn't preserve addresses
    assert_ne!(vec_address_before, vec_address_after);
    // pin_stack does
    assert_eq!(pin_stack_address_before, pin_stack_address_after);
}

#[test]
fn get() {
    let mut pin_stack = PinStack::new();

    for i in 0..100 {
        pin_stack.push(i);
    }

    assert_eq!(*pin_stack.get(99).unwrap(), 99);
    assert_eq!(pin_stack.get(100), None);
    assert_eq!(pin_stack.get(100000), None);

    assert_eq!(*pin_stack.get_mut(99).unwrap(), 99);
    assert_eq!(pin_stack.get_mut(100), None);
    assert_eq!(pin_stack.get_mut(100000), None);
}
