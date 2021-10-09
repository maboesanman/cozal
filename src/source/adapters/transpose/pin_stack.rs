use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ops::RangeBounds;
use core::pin::Pin;

pub struct PinStack<T: Sized> {
    length: usize,
    chunks: Vec<Box<[MaybeUninit<T>]>>,
}

fn get_pos(index: usize) -> (usize, usize) {
    let y = usize::BITS - index.leading_zeros();
    (y as usize, index - (1 << y >> 1))
}

fn chunk_size(chunk_index: usize) -> usize {
    ((1 << chunk_index) + 1) >> 1
}

#[allow(unused)]
impl<T: Sized> PinStack<T> {
    pub fn new() -> Self {
        Self {
            length: 0,
            chunks: Vec::new(),
        }
    }

    pub fn capacity(&self) -> usize {
        1 << self.chunks.len() >> 1
    }

    pub fn reserve(&mut self, additional: usize) {
        let new_highest_i = self.length + additional;
        if new_highest_i == 0 {
            return
        }
        let new_highest_i = new_highest_i - 1;

        let (max_chunk, _) = get_pos(new_highest_i);
        while max_chunk + 1 > self.chunks.len() {
            self.reserve_one_more_chunk()
        }
    }

    fn reserve_one_more_chunk(&mut self) {
        let next_chunk_i = self.chunks.len();
        let next_chunk_size = chunk_size(next_chunk_i);
        let new_slice = Box::new_uninit_slice(next_chunk_size);

        self.chunks.push(new_slice);
    }

    // SAFETY: index must be reserved, ignore whether or not item is initialized
    unsafe fn get_unchecked(&self, index: usize) -> &MaybeUninit<T> {
        let (chunk_i, i) = get_pos(index);

        debug_assert!(chunk_i < self.chunks.len());
        let chunk = self.chunks.get_unchecked(chunk_i);

        debug_assert!(i < chunk.len());
        chunk.get_unchecked(i)
    }

    // SAFETY: index must be reserved, ignore whether or not item is initialized
    unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut MaybeUninit<T> {
        let (chunk_i, i) = get_pos(index);

        debug_assert!(chunk_i < self.chunks.len());
        let chunk = self.chunks.get_unchecked_mut(chunk_i);

        debug_assert!(i < chunk.len());
        chunk.get_unchecked_mut(i)
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn empty(&self) -> bool {
        self.length == 0
    }

    pub fn push(&mut self, item: T) {
        self.reserve(1);
        self.length += 1;

        // SAFETY: index is reserved from above, and uninit, so we can replace it.
        let item_mut = unsafe { self.get_unchecked_mut(self.length - 1) };
        *item_mut = MaybeUninit::new(item);
    }

    pub fn pop(&mut self) -> bool {
        if self.length == 0 {
            false
        } else {
            self.length -= 1;
            // SAFETY: self.length is low enough because we just decreased it by one.
            let top_item = unsafe { self.get_unchecked_mut(self.length) };
            // SAFETY: top_item is init because it was at index self.length - 1 at the beginning of this function.
            unsafe { top_item.assume_init_drop() };
            *top_item = MaybeUninit::uninit();
            true
        }
    }

    // SAFETY: you are not allowed to move something which has ever been pinned, which is exactly what this does. The caller of this function must be careful to only do sound operations on the T they might get from it, including dropping it.
    pub unsafe fn pop_recover(&mut self) -> Option<T> {
        if self.length == 0 {
            None
        } else {
            self.length -= 1;
            let item_mut = self.get_unchecked_mut(self.length);
            let item = core::mem::replace(item_mut, MaybeUninit::uninit());
            let item = item.assume_init();
            Some(item)
        }
    }

    pub fn peek(&self) -> Option<&T> {
        if self.length == 0 {
            return None
        }

        self.get(self.length - 1)
    }

    pub fn peek_mut(&mut self) -> Option<Pin<&mut T>> {
        if self.length == 0 {
            return None
        }

        self.get_mut(self.length - 1)
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if self.length <= index {
            None
        } else {
            // SAFETY: We just checked that self.length > index.
            let item = unsafe { self.get_unchecked(index) };
            // SAFETY: Because this is inside length, we can assume it is init.
            let item = unsafe { item.assume_init_ref() };

            Some(item)
        }
    }

    pub fn get_mut(&mut self, index: usize) -> Option<Pin<&mut T>> {
        if self.length <= index {
            None
        } else {
            // SAFETY: We just checked that self.length > index.
            let item_mut = unsafe { self.get_unchecked_mut(index) };
            // SAFETY: Because this is inside length, we can assume it is init.
            let item_mut = unsafe { item_mut.assume_init_mut() };
            // SAFETY: We never move any items we own.
            let item_mut = unsafe { Pin::new_unchecked(item_mut) };

            Some(item_mut)
        }
    }

    pub fn range_by<K, F, R>(&self, range: R, func: F) -> RangeMutBy<'_, T, K, F>
    where
        K: Ord,
        F: Fn(&T) -> K,
        R: RangeBounds<K>,
    {
        RangeMutBy::new(self, range, func)
    }
}

impl<T: Sized> Drop for PinStack<T> {
    fn drop(&mut self) {
        // items need to be dropped in reverse order, because they may contain references to previous elements.
        while self.pop() {}
    }
}

pub struct RangeMutBy<'a, T: Sized, K, F>
where
    K: Ord,
    F: Fn(&T) -> K,
{
    pin_stack: &'a PinStack<T>,

    // index in array
    front: usize,
    back:  usize,
    done:  bool,

    _marker: PhantomData<&'a mut (K, F)>,
}

impl<'a, T: Sized, K, F> RangeMutBy<'a, T, K, F>
where
    K: Ord,
    F: Fn(&T) -> K,
{
    fn new<R>(pin_stack: &'a PinStack<T>, range: R, func: F) -> Self
    where
        R: RangeBounds<K>,
    {
        if pin_stack.length == 0 {
            return Self {
                pin_stack,
                front: 0,
                back: 0,
                done: true,

                _marker: PhantomData,
            }
        }
        let mut low = 0;
        let mut high = pin_stack.length - 1;
        let front = match range.start_bound() {
            core::ops::Bound::Included(x) => {
                while low < high {
                    let m = (low + high) / 2;
                    let k = func(pin_stack.get(m).unwrap());

                    match k.cmp(x) {
                        core::cmp::Ordering::Less => {
                            low = m + 1;
                        },
                        core::cmp::Ordering::Equal => {
                            high = m;
                        },
                        core::cmp::Ordering::Greater => {
                            high = m;
                        },
                    }
                }

                low
            },
            core::ops::Bound::Excluded(x) => {
                while low < high {
                    let m = (low + high) / 2;
                    let k = func(pin_stack.get(m).unwrap());

                    match k.cmp(x) {
                        core::cmp::Ordering::Less => {
                            low = m + 1;
                        },
                        core::cmp::Ordering::Equal => {
                            low = m + 1;
                        },
                        core::cmp::Ordering::Greater => {
                            high = m;
                        },
                    }
                }

                low
            },
            core::ops::Bound::Unbounded => low,
        };

        let mut low = 0;
        let mut high = pin_stack.length - 1;
        let back = match range.end_bound() {
            core::ops::Bound::Included(x) => {
                while low < high {
                    let m = (low + high + 1) / 2;
                    let k = func(pin_stack.get(m).unwrap());

                    match k.cmp(x) {
                        core::cmp::Ordering::Less => {
                            low = m;
                        },
                        core::cmp::Ordering::Equal => {
                            low = m;
                        },
                        core::cmp::Ordering::Greater => {
                            high = m - 1;
                        },
                    }
                }

                low
            },
            core::ops::Bound::Excluded(x) => {
                while low < high {
                    let m = (low + high + 1) / 2;
                    let k = func(pin_stack.get(m).unwrap());

                    match k.cmp(x) {
                        core::cmp::Ordering::Less => {
                            low = m;
                        },
                        core::cmp::Ordering::Equal => {
                            high = m - 1;
                        },
                        core::cmp::Ordering::Greater => {
                            high = m - 1;
                        },
                    }
                }

                low
            },
            core::ops::Bound::Unbounded => high,
        };

        if front > back {
            Self {
                pin_stack,
                front: 0,
                back: 0,
                done: true,

                _marker: PhantomData,
            }
        } else {
            Self {
                pin_stack,
                front,
                back,
                done: false,

                _marker: PhantomData,
            }
        }
    }
}

impl<'a, T: Sized, K, F> Iterator for RangeMutBy<'a, T, K, F>
where
    K: Ord,
    F: Fn(&T) -> K,
{
    type Item = (usize, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None
        }
        let index = self.front;
        self.front += 1;
        if self.front > self.back {
            self.done = true;
        }

        let item = self.pin_stack.get(index).unwrap();

        Some((index, item))
    }
}

impl<'a, T: Sized, K, F> DoubleEndedIterator for RangeMutBy<'a, T, K, F>
where
    K: Ord,
    F: Fn(&T) -> K,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.done {
            return None
        }
        let index = self.back;
        if self.back == 0 {
            self.done = true;
        } else {
            self.back -= 1;
            if self.front > self.back {
                self.done = true;
            }
        }

        let item = self.pin_stack.get(index).unwrap();

        Some((index, item))
    }
}
