use std::collections::BTreeMap;

use util::replace_mut;

use super::Transposer;

pub struct InputBuffer<T: Transposer>(BTreeMap<T::Time, InputContainer<T::Input>>);

enum InputContainer<Input> {
    Hot(Vec<Input>),
    Cold(Box<[Input]>),
}

impl<Input> InputContainer<Input> {
    pub fn push(&mut self, input: Input) {
        self.heat().push(input);
    }

    pub fn singleton(input: Input) -> Self {
        Self::Hot(vec![input])
    }

    fn heat(&mut self) -> &mut Vec<Input> {
        replace_mut::replace(self, Self::recover, |this| match this {
            InputContainer::Cold(b) => {
                let v = b.into_vec();
                InputContainer::Hot(v)
            },
            hot => hot,
        });

        match self {
            InputContainer::Hot(vec) => vec,
            InputContainer::Cold(_) => unreachable!(),
        }
    }

    fn recover() -> Self {
        Self::Cold(Box::new([]))
    }
}

impl<Input> From<Vec<Input>> for InputContainer<Input> {
    fn from(vec: Vec<Input>) -> Self {
        Self::Hot(vec)
    }
}

impl<Input> From<Box<[Input]>> for InputContainer<Input> {
    fn from(slice: Box<[Input]>) -> Self {
        Self::Cold(slice)
    }
}

impl<Input> From<InputContainer<Input>> for Box<[Input]> {
    fn from(from: InputContainer<Input>) -> Self {
        match from {
            InputContainer::Hot(vec) => vec.into_boxed_slice(),
            InputContainer::Cold(slice) => slice,
        }
    }
}

impl<Input> From<InputContainer<Input>> for Vec<Input> {
    fn from(val: InputContainer<Input>) -> Self {
        match val {
            InputContainer::Hot(vec) => vec,
            InputContainer::Cold(slice) => slice.into(),
        }
    }
}

impl<Input> IntoIterator for InputContainer<Input> {
    type Item = Input;
    type IntoIter = <Vec<Input> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        let v: Vec<_> = self.into();
        v.into_iter()
    }
}

impl<T: Transposer> InputBuffer<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_back(&mut self, time: T::Time, input: T::Input) {
        match self.0.get_mut(&time) {
            Some(current) => current.push(input),
            None => {
                self.0.insert(time, InputContainer::singleton(input));
            },
        }
    }

    pub fn extend_front(&mut self, time: T::Time, inputs: Box<[T::Input]>) {
        match self.0.get_mut(&time) {
            Some(current) => replace_mut::replace(current, InputContainer::recover, |c| {
                let mut new_vec: Vec<_> = inputs.into();
                new_vec.extend(c.into_iter());
                new_vec.into()
            }),
            None => {
                self.0.insert(time, inputs.into());
            },
        }
    }

    pub fn first_time(&self) -> Option<T::Time> {
        self.0.first_key_value().map(|(&k, _)| k)
    }

    pub fn pop_first(&mut self) -> Option<(T::Time, Box<[T::Input]>)> {
        self.0.pop_first().map(|(t, v)| (t, v.into()))
    }

    pub fn rollback(&mut self, time: T::Time) {
        let InputBuffer(inner) = self;
        inner.split_off(&time);
    }
}

impl<T: Transposer> Default for InputBuffer<T> {
    fn default() -> Self {
        Self(BTreeMap::new())
    }
}
