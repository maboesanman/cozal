use std::{marker::PhantomData, mem::MaybeUninit, pin::Pin};

use super::pin_stack::PinStack;

pub struct SparseBufferStack<'stack, I: Sized + 'stack, B: Clone + 'stack, const N: usize> {
    stack: PinStack<StackItem<I>>,
    buffer: [BufferItem<'stack, I, B>; N],

    _marker: PhantomData<&'stack I>,
}

impl<'stack, I: Sized + 'stack, B: Clone + 'stack, const N: usize> SparseBufferStack<'stack, I, B, N> {
    #[allow(unused)]
    pub fn new<F>(item: I, constructor: F) -> Self
        where F: FnOnce(&'stack I) -> B
    {
        let mut stack = PinStack::new();
        stack.push(StackItem {
            buffer_index: 0,
            item
        });
        let stack_item = stack.peek().unwrap();
        let stack_item = stack_item as *const StackItem<I>;

        // SAFETY: the buffer is always dropped before the item, so this is safe.
        let stack_item: &'stack StackItem<I> = unsafe { stack_item.as_ref().unwrap() };
        let mut buffer: [BufferItem<'stack, I, B>; N] = array_init::array_init(|_| BufferItem::new_zeroed());
        buffer[0].replace_with(0, constructor(&stack_item.item));

        Self {
            stack,
            buffer,

            _marker: PhantomData,
        }
    }

    #[allow(unused)]
    pub fn push<F>(&mut self, constructor: F)
        where F: FnOnce(&'stack I) -> I
    {
        // this is fine cause we don't allow stack to hit 0 elements ever.
        let top = &self.stack.peek().unwrap().item;
        let top = top as *const I;

        // SAFETY: because this is a stack, subsequent values will be dropped first.
        let top: &'stack I = unsafe { top.as_ref().unwrap() };
        let item = constructor(&top);
        self.stack.push(StackItem {
            buffer_index: 0,
            item
        });
    }

    // constructor takes a reference to the stack item, and a reference to the previous buffered item.
    #[allow(unused)]
    pub fn buffer<F>(&mut self, stack_index: usize, constructor: F) -> Result<(), ()>
        where F: FnOnce(&'stack I, &mut B),
    {
        let buffer_index_to_replace = self.get_least_useful_buffer_index();
        
        let stack_item = self.stack.get_mut(stack_index).ok_or(())?;
        let stack_item = unsafe { stack_item.get_unchecked_mut() };
        stack_item.buffer_index = buffer_index_to_replace;
        let stack_item = stack_item as *const StackItem<I>;

        // SAFETY: the buffer is always dropped before the item, so this is safe.
        let stack_item: &'stack StackItem<I> = unsafe { stack_item.as_ref().unwrap() };

        let (before, remaining) = self.buffer.split_at_mut(buffer_index_to_replace);
        let (buffer_item, after) = remaining.split_first_mut().unwrap();

        if buffer_item.stack_index != stack_index - 1 {
            let prev_stack_item = self.stack.get(stack_index - 1).unwrap();
            let prev_buffer_item = if prev_stack_item.buffer_index < buffer_index_to_replace {
                before.get(prev_stack_item.buffer_index).unwrap()
            } else {
                after.get(prev_stack_item.buffer_index - buffer_index_to_replace - 1).unwrap()
            };
            let item = prev_buffer_item.get_buffer(stack_index - 1).ok_or(())?;

            buffer_item.replace_with(stack_index, item.clone());
        } else {
            buffer_item.stack_index = stack_index;
        }

        constructor(&&stack_item.item, unsafe { buffer_item.item.assume_init_mut()});

        Ok(())
    }

    pub fn get_buffered(&self, stack_index: usize) -> Option<&B> {
        if stack_index == usize::MAX {
            panic!("there definitely aren't usize::MAX items in this collection...")
        }

        let stack_item = self.stack.get(stack_index)?;
        let buffer_item = self.buffer.get(stack_item.buffer_index)?;
        buffer_item.get_buffer(stack_index)
    }

    pub fn get_pinned_buffered(self: Pin<&mut Self>, stack_index: usize) -> Option<Pin<&mut B>> {
        if stack_index == usize::MAX {
            panic!("there definitely aren't usize::MAX items in this collection...")
        }

        // SAFETY: we never move the buffered item after making it, so we can safely return a pinned pointer to it.
        let this = unsafe { self.get_unchecked_mut() };

        let stack_item = this.stack.get(stack_index)?;
        let buffer_item = this.buffer.get_mut(stack_item.buffer_index)?;
        let buf_ref = buffer_item.get_buffer_mut(stack_index)?;
        
        // SAFETY: we never move the buffered item after making it, so we can safely return a pinned pointer to it.
        Some(unsafe { Pin::new_unchecked(buf_ref) })
    } 

    #[allow(unused)]
    pub fn pop(&mut self) -> Option<I> {
        if self.stack.len() > 1 {
            let stack_index = self.stack.len() - 1;
            let buffer_index = self.stack.peek().unwrap().buffer_index;
            self.drop_buffered(buffer_index, stack_index);

            Some(self.stack.pop().unwrap().item)
        } else {
            None
        }
    }

    fn drop_buffered(&mut self, buffer_index: usize, expected_stack_index: usize) {
        if expected_stack_index == usize::MAX {
            panic!("tried to drop an empty StateBufferItem")
        }
        
        if let Some(item) = self.buffer.get_mut(buffer_index) {
            if expected_stack_index == item.stack_index {
                unsafe { item.assume_init_drop() }
            }
        }
    }

    // get the buffer_index and stack_index of the next item to delete.
    fn get_least_useful_buffer_index(&self) -> usize {
        let iter = self.buffer.iter().enumerate();
        let index = (self.stack.len() - 1) as isize;
        let (buffer_index, _) = iter.min_by_key(|&(_, x)| (index - x.stack_index as isize).leading_zeros() + x.stack_index.trailing_zeros()).unwrap();

        buffer_index
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }
}

struct StackItem<I: Sized> {
    buffer_index: usize,
    item: I,
}

impl<I: Sized> StackItem<I> {
    
}

struct BufferItem<'stack, I: Sized + 'stack, B: Sized + 'stack> {
    // usize::MAX marks this as an empty buffer.
    stack_index: usize,
    item: MaybeUninit<B>,

    _marker: PhantomData<&'stack I>,
}

impl<'stack, I: Sized + 'stack, B: Sized + 'stack> BufferItem<'stack, I, B> {
    pub fn new_zeroed() -> Self {
        Self {
            stack_index: usize::MAX,
            item: MaybeUninit::uninit(),

            _marker: PhantomData,
        }
    }

    fn new(index: usize, item: B) -> Self {
        BufferItem {
            stack_index: index,
            item: MaybeUninit::new(item),

            _marker: PhantomData,
        }
    }

    pub fn get_buffer_mut(&mut self, stack_index: usize) -> Option<&mut B> {
        if self.stack_index != stack_index {
            None
        } else {
            // SAFETY: buffer_item.stack_index can't be usize::MAX here so it must be init.
            Some(unsafe { self.item.assume_init_mut() })
        }
    }

    pub fn get_buffer(&self, stack_index: usize) -> Option<&B> {
        if self.stack_index != stack_index {
            None
        } else {
            // SAFETY: buffer_item.stack_index can't be usize::MAX here so it must be init.
            Some(unsafe { self.item.assume_init_ref() })
        }
    }

    unsafe fn assume_init_drop(&mut self) {
        self.stack_index = usize::MAX;
        self.item.assume_init_drop()
    }

    pub fn drop_if_init(&mut self) {
        if self.stack_index != usize::MAX {
            unsafe { self.assume_init_drop() };
        }
    }

    pub fn replace_with(&mut self, stack_index: usize, item: B) {
        if self.stack_index != usize::MAX {
            unsafe { self.item.assume_init_drop() };
        }
        self.stack_index = stack_index;
        self.item = MaybeUninit::new(item);
    }
}
