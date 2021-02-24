use std::collections::BTreeMap;

pub struct InputBuffer<Time: Ord + Copy, Input>(BTreeMap<Time, Vec<Input>>);

impl<Time: Ord + Copy, Input> InputBuffer<Time, Input> {
    pub fn new() -> Self {
        InputBuffer(BTreeMap::new())
    }

    pub fn insert_back(&mut self, time: Time, input: Input) {
        match self.0.get_mut(&time) {
            Some(current) => current.push(input),
            None => {self.0.insert(time, vec![input]);},
        }
    }

    pub fn extend_front(&mut self, time: Time, mut inputs: Vec<Input>) {
        match self.0.get_mut(&time) {
            Some(current) => {
                inputs.extend(current.drain(..));
                *current = inputs;
            },
            None => {self.0.insert(time, inputs);},
        }
    }

    pub fn first_time(&self) -> Option<Time> {
        self.0.first_key_value().map(|(&k, _)| k)
    }

    pub fn pop_first(&mut self) -> Option<(Time, Vec<Input>)> {
        self.0.pop_first()
    }
}