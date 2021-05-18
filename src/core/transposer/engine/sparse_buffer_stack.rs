use std::{marker::PhantomData, mem::MaybeUninit, pin::Pin};

use super::pin_stack::PinStack;
use pin_project::pin_project;

pub struct SparseBufferStack<'stack, I: Sized + 'stack, B: Sized + 'stack, const N: usize> {
    stack: PinStack<StackItem<I>>,
    buffer: [BufferItem<'stack, I, B>; N],

    _marker: PhantomData<&'stack I>,
}

impl<'stack, I: Sized + 'stack, B: Sized + 'stack, const N: usize>
    SparseBufferStack<'stack, I, B, N>
{
    pub fn new<Con, Init>(stack_item: I, constructor: Con, initializer: Init) -> Self
    where
        Con: FnOnce(&'stack I) -> B,
        Init: FnOnce(Pin<&mut B>, &'stack I),
    {
        let mut stack = PinStack::new();
        stack.push(StackItem {
            buffer_index: 0,
            item: stack_item,
        });
        let stack_item_ref = stack.peek().unwrap();
        let stack_item_ref = stack_item_ref as *const StackItem<I>;

        // SAFETY: the buffer is always dropped before the item, so this is safe.
        let stack_item_ref: &'stack StackItem<I> = unsafe { stack_item_ref.as_ref().unwrap() };
        let stack_item_ref = &stack_item_ref.item;

        let mut buffer: [BufferItem<'stack, I, B>; N] =
            array_init::array_init(|_| BufferItem::new_zeroed());
        buffer[0].replace_with(0, constructor(stack_item_ref));
        let buffer_item_ref = buffer.get_mut(0).unwrap();

        // SAFETY: we just assigned this so it must exist.
        let buffer_item_ref = unsafe { buffer_item_ref.item.assume_init_mut() };

        // SAFETY: this buffer will never move so we can pin it.
        let buffer_item_ref = unsafe { Pin::new_unchecked(buffer_item_ref) };

        initializer(buffer_item_ref, stack_item_ref);

        Self {
            stack,
            buffer,

            _marker: PhantomData,
        }
    }

    pub fn push<Con>(self: Pin<&mut Self>, constructor: Con)
    where
        Con: FnOnce(&'stack I) -> I,
    {
        // SAFETY: we don't touch the buffered items here.
        let this = unsafe { self.get_unchecked_mut() };

        // this is fine cause we don't allow stack to hit 0 elements ever.
        let top = &this.stack.peek().unwrap().item;
        let top = top as *const I;

        // SAFETY: because this is a stack, subsequent values will be dropped first.
        let top: &'stack I = unsafe { top.as_ref().unwrap() };
        let item = constructor(&top);
        this.stack.push(StackItem {
            buffer_index: 0,
            item,
        });
    }

    // pub fn push_from_buffer<Con>(self: Pin<&mut Self>, constructor: Con) -> Result<(), ()>
    //     where Con: FnOnce(&'stack I, Pin<&mut B>) -> I
    // {
    //     // SAFETY: we don't touch the buffered items here.
    //     let this = unsafe { self.get_unchecked_mut() };

    //     // this is fine cause we don't allow stack to hit 0 elements ever.
    //     let top = &this.stack.peek().unwrap().item;
    //     let top = top as *const I;

    //     // SAFETY: because this is a stack, subsequent values will be dropped first.
    //     let top: &'stack I = unsafe { top.as_ref().unwrap() };
    //     let item = constructor(&top);
    //     this.stack.push(StackItem {
    //         buffer_index: 0,
    //         item
    //     });
    // }

    // constructor takes a reference to the stack item, and a reference to the previous buffered item.
    pub fn buffer<Dup, Refurb, Init>(
        self: Pin<&mut Self>,
        stack_index: usize,
        duplicator: Dup,
        refurbisher: Refurb,
        initializer: Init,
    ) -> Result<(), ()>
    where
        Dup: FnOnce(&B) -> B, // reference to previous buffered item, use it to create a new one
        Refurb: FnOnce(&mut B), // reference to previous buffered item, prepare for in place updates
        Init: FnOnce(Pin<&mut B>, &'stack I), // reference to stack item and pinned mut reference to current buffer
    {
        // SAFETY: we don't move the buffered items here; just drop them.
        let this = unsafe { self.get_unchecked_mut() };

        let buffer_index_to_replace = this.get_least_useful_buffer_index();

        let stack_item = this.stack.get_mut(stack_index).ok_or(())?;
        let stack_item = unsafe { stack_item.get_unchecked_mut() };
        stack_item.buffer_index = buffer_index_to_replace;
        let stack_item = stack_item as *const StackItem<I>;

        // SAFETY: the buffer is always dropped before the item, so this is safe.
        let stack_item: &'stack StackItem<I> = unsafe { stack_item.as_ref().unwrap() };

        let (before, remaining) = this.buffer.split_at_mut(buffer_index_to_replace);
        let (buffer_item, after) = remaining.split_first_mut().unwrap();

        if buffer_item.stack_index != stack_index - 1 {
            let prev_stack_item = this.stack.get(stack_index - 1).unwrap();
            let prev_buffer_item = if prev_stack_item.buffer_index < buffer_index_to_replace {
                before.get(prev_stack_item.buffer_index).unwrap()
            } else {
                after
                    .get(prev_stack_item.buffer_index - buffer_index_to_replace - 1)
                    .unwrap()
            };
            let item = prev_buffer_item.get_buffer(stack_index - 1).ok_or(())?;

            buffer_item.replace_with(stack_index, duplicator(item));
        } else {
            buffer_item.stack_index = stack_index;
            refurbisher(buffer_item.get_buffer_mut(stack_index).unwrap());
        }

        initializer(
            unsafe { Pin::new_unchecked(buffer_item.item.assume_init_mut()) },
            &&stack_item.item,
        );

        Ok(())
    }

    pub fn get(&self, stack_index: usize) -> Option<(&I, Option<&B>)> {
        let stack_item = self.stack.get(stack_index)?;
        let buffer_item = self.buffer.get(stack_item.buffer_index);
        let stack_item = &stack_item.item;
        let buffer_item = buffer_item.map(|x| x.get_buffer(stack_index)).flatten();
        Some((stack_item, buffer_item))
    }

    pub fn get_pinned_mut(
        self: Pin<&mut Self>,
        stack_index: usize,
    ) -> Option<(Pin<&mut I>, Option<Pin<&mut B>>)> {
        // SAFETY: we never move the buffered item after making it, so we can safely return a pinned pointer to it.
        let this = unsafe { self.get_unchecked_mut() };

        let stack_item = this.stack.get_mut(stack_index)?;
        let buffer_item = this.buffer.get_mut(stack_item.buffer_index);
        let stack_item = stack_item.project().item;
        let buffer_item = buffer_item.map(|x| x.get_buffer_mut(stack_index)).flatten();
        // SAFETY: we never move the buffered item after making it, so we can safely return a pinned pointer to it.
        let buffer_item = buffer_item.map(|x| unsafe { Pin::new_unchecked(x) });

        Some((stack_item, buffer_item))
    }

    pub fn last_buffered_index_by<K, F>(&self, reference: K, func: F) -> usize
    where
        K: Ord,
        F: Fn(&I) -> K,
    {
        let range = self
            .stack
            .range_by(..=reference, |stack_item| func(&stack_item.item));
        let (last_buffered_stack_index, _) = range
            .rev()
            .find(|(stack_index, stack_item)| {
                // this stack item has a buffered state
                self.buffer
                    .get(stack_item.buffer_index)
                    .unwrap()
                    .stack_index
                    == *stack_index
            })
            .unwrap();

        last_buffered_stack_index
    }

    pub fn last_index_by<K, F>(&self, reference: K, func: F) -> usize
    where
        K: Ord,
        F: Fn(&I) -> K,
    {
        let range = self
            .stack
            .range_by(..=reference, |stack_item| func(&stack_item.item));
        let (index, _) = range.last().unwrap();
        index
    }

    pub fn find<F>(&self, func: F) -> Option<&I>
    where
        F: Fn(&I) -> bool,
    {
        let mut range = self.stack.range_by(.., |stack_item| 1);
        range
            .find(|(_, item)| func(&item.item))
            .map(|(_, item)| &item.item)
    }

    pub fn pop(self: Pin<&mut Self>) -> Option<I> {
        // SAFETY: we don't move the buffered items here; just drop them.
        let this = unsafe { self.get_unchecked_mut() };

        if this.stack.len() > 1 {
            let stack_index = this.stack.len() - 1;
            let buffer_index = this.stack.peek().unwrap().buffer_index;
            this.drop_buffered(buffer_index, stack_index);

            Some(this.stack.pop().unwrap().item)
        } else {
            None
        }
    }

    pub fn peek(&self) -> &I {
        &self.stack.peek().unwrap().item
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
        let (buffer_index, _) = iter
            .min_by_key(|&(_, x)| {
                (index - x.stack_index as isize).leading_zeros() + x.stack_index.trailing_zeros()
            })
            .unwrap();

        buffer_index
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }
}

#[pin_project]
struct StackItem<I: Sized> {
    buffer_index: usize,

    #[pin]
    item: I,
}

impl<I: Sized> StackItem<I> {}

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
        if stack_index == usize::MAX {
            panic!("stack_index too large");
        }

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
