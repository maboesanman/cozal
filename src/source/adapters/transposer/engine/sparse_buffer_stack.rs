use core::{marker::PhantomData, mem::MaybeUninit, pin::Pin};

use super::pin_stack::PinStack;
use pin_project::pin_project;

#[pin_project]
pub struct SparseBufferStack<
    'stack,
    I: 'stack,
    B: 'stack,
    const N: usize,
> {
    needs_init: Option<Box<dyn 'stack + FnOnce(&'stack I) -> B>>,
    // this always has at least one item in it. pop doesn't let you pop the first item.
    stack: PinStack<StackItem<I>>,

    #[pin]
    buffer: [BufferItem<'stack, I, B>; N],

    _marker: PhantomData<&'stack I>,
}

#[allow(unused)]
impl<'stack, I: 'stack, B: 'stack, const N: usize>
    SparseBufferStack<'stack, I, B, N>
{
    pub fn new<Init>(first_item: I, constructor: Init) -> Self
    where
        Init: 'stack + FnOnce(&'stack I) -> B
    {
        let mut stack = PinStack::new();
        stack.push(StackItem {
            buffer_index: 0,
            item: first_item,
        });

        let buffer: [BufferItem<'stack, I, B>; N] =
            array_init::array_init(|_| BufferItem::new_zeroed());

        Self {
            needs_init: Some(Box::new(constructor)),
            stack,
            buffer,

            _marker: PhantomData,
        }
    }

    fn ensure_init(self: Pin<&mut Self>) {
        let this = self.project();
        if this.needs_init.is_none() {
            return;
        }

        let constructor = core::mem::take(this.needs_init).unwrap();
        let stack_item_ref = this.stack.peek().unwrap();
        let stack_item_ref = stack_item_ref as *const StackItem<I>;

        // SAFETY: the buffer is always dropped before the item, so this is safe.
        let stack_item_ref: &'stack StackItem<I> = unsafe { stack_item_ref.as_ref().unwrap() };
        let stack_item_ref = &stack_item_ref.item;

        // SAFETY: structural pinning of an array
        let buffer_item_pinned_ref = unsafe {
            let buffer = this.buffer.get_unchecked_mut();
            let item = &mut buffer[0];
            Pin::new_unchecked(item)
        };

        buffer_item_pinned_ref.replace_with(0, constructor(stack_item_ref));
    }

    // SAFETY: the closure must not move the supplied item reference anywhere but B
    pub fn push<Con>(mut self: Pin<&mut Self>, constructor: Con)
    where
        Con: FnOnce(&'stack I) -> I,
    {
        self.as_mut().ensure_init();
        let this = self.project();
        debug_assert!(this.needs_init.is_none());

        // this is fine cause we don't allow stack to hit 0 elements ever.
        let top = &this.stack.peek().unwrap().item;
        let top = top as *const I;

        // SAFETY: because this is a pin_stack, subsequent values will be dropped first, and nothing will move.
        let top: &'stack I = unsafe { top.as_ref().unwrap() };
        let item = constructor(&top);
        this.stack.push(StackItem {
            buffer_index: 0,
            item,
        });
    }

    // SAFETY: the closure must not move the supplied item reference anywhere but B
    pub fn buffer<Dup, Refurb>(
        mut self: Pin<&mut Self>,
        stack_index: usize,
        duplicator: Dup,
        refurbisher: Refurb,
    ) -> Result<(&I, Pin<&mut B>), ()>
    where
        Dup: FnOnce(&B, &'stack I) -> B, // reference to previous buffered item, use it to create a new one
        Refurb: FnOnce(Pin<&mut B>, &'stack I), // reference to previous buffered item, prepare for in place updates
    {
        self.as_mut().ensure_init();
        let buffer_index_to_replace = self.get_least_useful_buffer_index();
        
        let this = self.project();
        debug_assert!(this.needs_init.is_none());

        let prev_stack_item_buffer_index = this.stack.get(stack_index - 1).ok_or(())?.buffer_index;
        let mut stack_item = this.stack.get_mut(stack_index).ok_or(())?;
        let stack_item = unsafe { stack_item.get_unchecked_mut() };
        stack_item.buffer_index = buffer_index_to_replace;

        // SAFETY: structural pinning of an array.
        let (before, mut buffer_item_pinned_ref, after) = unsafe {
            let buffer = unsafe { this.buffer.get_unchecked_mut() };
            let (before, remaining) = buffer.split_at_mut(buffer_index_to_replace);
            let (buffer_item, after) = remaining.split_first_mut().unwrap();

            (
                Pin::new_unchecked(before),
                Pin::new_unchecked(buffer_item),
                Pin::new_unchecked(after),
            )
        };

        let stack_item_ptr: *const I = &stack_item.item;
        // SAFETY: this ref is going to be passed to duplicator/refurbisher, and always dropped before its target due to the stack gurantees.
        let stack_item_ref: &'stack I = unsafe { stack_item_ptr.as_ref().unwrap() };

        let buffer_item = buffer_item_pinned_ref.as_mut().project();
        if *buffer_item.stack_index != stack_index - 1 {
            let prev_buffer_item = if prev_stack_item_buffer_index < buffer_index_to_replace {
                before.get(prev_stack_item_buffer_index).unwrap()
            } else {
                after
                    .get(prev_stack_item_buffer_index - buffer_index_to_replace - 1)
                    .unwrap()
            };
            let item = prev_buffer_item.get_buffer(stack_index - 1).ok_or(())?;
            buffer_item_pinned_ref.as_mut().replace_with(stack_index, duplicator(item, stack_item_ref));
        } else {
            *buffer_item.stack_index = stack_index;
            refurbisher(
                buffer_item_pinned_ref.as_mut().get_buffer_mut(stack_index).unwrap(),
                stack_item_ref,
            );
        }
        let stack_item: &I = &this.stack.get(stack_index).ok_or(())?.item;
        
        let mut buffer_item = buffer_item_pinned_ref.project();

        let buffer_item: Pin<&mut MaybeUninit<B>> = buffer_item.item;
        let buffer_item = unsafe {
            let x = buffer_item.get_unchecked_mut();
            let x = x.assume_init_mut();
            Pin::new_unchecked(x)
        };
        
        Ok((stack_item, buffer_item))
    }

    pub fn get(&self, stack_index: usize) -> Option<&I> {
        let stack_item = self.stack.get(stack_index)?;
        Some(&stack_item.item)
    }

    pub fn get_pinned_mut(
        mut self: Pin<&mut Self>,
        stack_index: usize,
    ) -> Option<(&I, Option<Pin<&mut B>>)> {
        self.as_mut().ensure_init();
        let this = self.project();
        debug_assert!(this.needs_init.is_none());

        let stack_item = this.stack.get(stack_index)?;
        
        // SAFETY: Structural pinning of array.
        let buffer_item = unsafe {
            let buffer = this.buffer.get_unchecked_mut();
            let buffer_item = buffer.get_mut(stack_item.buffer_index);
            buffer_item.map(|x| Pin::new_unchecked(x))
        };

        let stack_item = &stack_item.item;

        let buffer_item = buffer_item.map(|x| x.get_buffer_mut(stack_index)).flatten();

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
        let last_buffered_stack_index = range
            .rev()
            .find(|(stack_index, stack_item)| {
                // this stack item has a buffered state
                self.buffer
                    .get(stack_item.buffer_index)
                    .unwrap()
                    .stack_index
                    == *stack_index
            })
            .map(|(x, y)| x)
            .unwrap_or(0);

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

    pub fn can_pop(&self) -> bool {
        self.stack.len() > 1
    }

    pub fn pop(mut self: Pin<&mut Self>) -> bool {
        self.as_mut().ensure_init();
        let this = self.as_mut().project();
        debug_assert!(this.needs_init.is_none());

        if this.stack.len() > 1 {
            let stack_index = this.stack.len() - 1;
            let buffer_index = this.stack.peek().unwrap().buffer_index;
            self.as_mut().drop_buffered(buffer_index, stack_index);
            self.project().stack.pop();
            true
        } else {
            false
        }
    }

    pub unsafe fn pop_recover(mut self: Pin<&mut Self>) -> Option<I> {
        self.as_mut().ensure_init();
        let this = self.as_mut().project();
        debug_assert!(this.needs_init.is_none());

        if this.stack.len() > 1 {
            let stack_index = this.stack.len() - 1;
            let buffer_index = this.stack.peek().unwrap().buffer_index;
            self.as_mut().drop_buffered(buffer_index, stack_index);
            Some(self.project().stack.pop_recover().unwrap().item)
        } else {
            None
        }
    }

    pub fn peek(&self) -> &I {
        &self.stack.peek().unwrap().item
    }

    pub fn peek_pinned_mut(mut self: Pin<&mut Self>) -> Pin<&mut I> {
        self.as_mut().ensure_init();
        let this = self.project();
        debug_assert!(this.needs_init.is_none());

        this.stack.peek_mut().unwrap().project().item
    }

    fn drop_buffered(self: Pin<&mut Self>, buffer_index: usize, expected_stack_index: usize) {
        let this = self.project();
        if expected_stack_index == usize::MAX {
            panic!("tried to drop an empty StateBufferItem")
        }

        // SAFETY: structural pinning of buffer
        let buffer_item = unsafe {
            let buffer = this.buffer.get_unchecked_mut();
            let buffer_item = buffer.get_mut(buffer_index);
            buffer_item.map(|x| Pin::new_unchecked(x))
        };

        if let Some(item) = buffer_item {
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

#[pin_project]
struct BufferItem<'stack, I: 'stack, B: 'stack> {
    // usize::MAX marks this as an empty buffer.
    stack_index: usize,
    
    #[pin]
    item: MaybeUninit<B>,

    _marker: PhantomData<&'stack I>,
}

impl<'stack, I: 'stack, B: 'stack> BufferItem<'stack, I, B> {
    pub fn new_zeroed() -> Self {
        Self {
            stack_index: usize::MAX,
            item: MaybeUninit::uninit(),

            _marker: PhantomData,
        }
    }

    pub fn get_buffer_mut(self: Pin<&mut Self>, stack_index: usize) -> Option<Pin<&mut B>> {
        let this = self.project();
        if *this.stack_index != stack_index {
            None
        } else {
            // SAFETY: buffer_item.stack_index can't be usize::MAX here so it must be init. Also pin shuffling.
            Some(unsafe {
                let item = this.item.get_unchecked_mut();
                let item = item.assume_init_mut();
                Pin::new_unchecked(item)
            })
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

    unsafe fn assume_init_drop(self: Pin<&mut Self>) {
        let this = self.project();
        *this.stack_index = usize::MAX;
        this.item.get_unchecked_mut().assume_init_drop();
    }

    pub fn replace_with(self: Pin<&mut Self>, stack_index: usize, item: B) {
        let mut this = self.project();
        if *this.stack_index != usize::MAX {
            let item_mut = unsafe { this.item.as_mut().get_unchecked_mut() };
            unsafe { item_mut.assume_init_drop() };
        }
        *this.stack_index = stack_index;
        let item_mut = unsafe { this.item.as_mut().get_unchecked_mut() };
        *item_mut = MaybeUninit::new(item);
    }
}
