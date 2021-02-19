use std::{marker::PhantomData, mem::MaybeUninit, usize};
use std::fmt::Debug;
use std::pin::Pin;
use pin_project::pin_project;

// this is a type which holds a bunch of T, and decides which to keep based on how new they are and how spaced they are.
#[pin_project(project=DynamicBufferProject)]
pub struct DynamicBuffer<T: Sized, const N: usize> {
    #[pin]
    buffer: [BufferItem<T>; N],
    max_index: usize,
}

impl<T: Sized, const N: usize> DynamicBuffer<T, N> {
    pub fn new() -> Self {
        Self {
            buffer: array_init::array_init(|_| BufferItem::new_zeroed()),
            max_index: 0,
        }
    }

    fn insert_index<'a>(self: Pin<&mut Self>, item: T, new_index: usize, important_indices: &[usize]) -> *mut BufferItem<T>{
        let project = self.project();
        let buffer: Pin<&mut [BufferItem<T>; N]> = project.buffer;

        let iter = buffer.iter().enumerate();
        let (buffer_index, _) = iter.min_by_key(|&(_, x)| {
            let utility = important_indices.iter().map(
                |&index| (index as isize - x.index as isize
            ).leading_zeros()).max().unwrap_or(0);
            let utility = utility + x.index.trailing_zeros();
            utility
        }).unwrap();

        let buffer = unsafe { buffer.get_unchecked_mut()};
        buffer[buffer_index] = BufferItem::new(new_index, item);
        
        unsafe { buffer.get_unchecked_mut(buffer_index) }
    }

    // this replaces old values with the new inserted value.
    pub fn insert<'a>(self: Pin<&mut Self>, item: T, new_index: usize, important_indices: &[usize]) -> BufferPointer<T>{
        BufferPointer {
            index: new_index,
            ptr: self.insert_index(item, new_index, important_indices),

            phantom: PhantomData       
        }
    }

    pub fn reinsert<'a>(self: Pin<&mut Self>, ptr: &mut BufferPointer<T>, item: T, important_indices: &[usize]) -> Result<(), T> {
        if ptr.access().is_err() {
            return Err(item);
        }
        ptr.ptr = self.insert_index(item, ptr.index, important_indices);
        Ok(())
    }
}

struct BufferItem<T: Sized> {
    index: usize,
    item: MaybeUninit<T>,
}

impl<T: Sized> BufferItem<T> {
    pub fn new_zeroed() -> Self {
        Self {
            index: usize::MAX,
            item: MaybeUninit::uninit()
        }
    }

    fn new(index: usize, item: T) -> Self {
        BufferItem {
            index,
            item: MaybeUninit::new(item),
        }
    }

    unsafe fn assume_init_drop(&mut self) {
        self.index = usize::MAX;
        self.item.assume_init_drop()
    }
}

impl<T: Sized> Drop for BufferItem<T> {
    fn drop(&mut self) {
        if self.index != usize::MAX {
            unsafe { self.assume_init_drop() }
        }
    }
}

impl<T: Sized + Debug> Debug for BufferItem<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.index == usize::MAX {
            f.write_str("null")
        } else {
            let s = format!("{:?}: {:?}", self.index, unsafe { self.item.assume_init_ref() });
            f.write_str(&s)
        }
    }
}

// this is the only means of accessing items in the dynamic buffer
pub struct BufferPointer<T: Sized> {
    index: usize,
    ptr: *mut BufferItem<T>,

    phantom: PhantomData<T>
}

impl<T: Sized> BufferPointer<T> {
    fn access_inner(&mut self) -> Result<&mut BufferItem<T>, ()> {
        let buffer_item = unsafe { self.ptr.as_mut().unwrap() };
        if buffer_item.index == self.index {
            Ok(buffer_item)
        } else {
            Err(())
        }
    }
    pub fn access(&mut self) -> Result<Pin<&mut T>, ()> {
        Ok(unsafe { Pin::new_unchecked(self.access_inner()?.item.assume_init_mut()) } )
    }
}

impl<T: Sized> Drop for BufferPointer<T> {
    fn drop(&mut self) {
        if let Ok(buffer_item) = self.access_inner() {
            unsafe { buffer_item.assume_init_drop() }
        }
    }
}
