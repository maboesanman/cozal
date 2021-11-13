use std::collections::BTreeMap;

pub struct InputBuffer<Time: Ord + Copy, Input>(BTreeMap<Time, InputContainer<Input>>);

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
        take_mut::take(self, |this| match this {
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

impl<Time: Ord + Copy, Input> InputBuffer<Time, Input> {
    pub fn new() -> Self {
        InputBuffer(BTreeMap::new())
    }

    pub fn insert_back(&mut self, time: Time, input: Input) {
        match self.0.get_mut(&time) {
            Some(current) => current.push(input),
            None => {
                self.0.insert(time, InputContainer::singleton(input));
            },
        }
    }

    pub fn extend_front(&mut self, time: Time, inputs: Box<[Input]>) {
        match self.0.get_mut(&time) {
            Some(current) => take_mut::take(current, |c| {
                let mut new_vec: Vec<_> = inputs.into();
                new_vec.extend(c.into_iter());
                new_vec.into()
            }),
            None => {
                self.0.insert(time, inputs.into());
            },
        }
    }

    pub fn first_time(&self) -> Option<Time> {
        self.0.first_key_value().map(|(&k, _)| k)
    }

    pub fn pop_first(&mut self) -> Option<(Time, Box<[Input]>)> {
        self.0.pop_first().map(|(t, v)| (t, v.into()))
    }

    pub fn rollback(&mut self, time: Time) {
        let InputBuffer(inner) = self;
        inner.split_off(&time);
    }
}
