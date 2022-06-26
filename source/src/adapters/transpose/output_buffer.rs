use std::collections::{BTreeMap, VecDeque};

use transposer::Transposer;

use crate::source_poll::SourcePollOk;

pub struct OutputBuffer<T: Transposer>(BTreeMap<T::Time, VecDeque<OutputType<T>>>);

pub enum OutputType<T: Transposer> {
    Event(T::Output),
    Finalize,
    Rollback,
}

impl<T: Transposer> OutputBuffer<T> {
    pub fn new() -> Self {
        OutputBuffer(BTreeMap::new())
    }
    pub fn poll(&mut self, time: T::Time) -> SourcePollOk<T::Time, T::Output, ()> {
        let (&t, vec) = match self.0.first_key_value() {
            Some(x) => x,
            None => return SourcePollOk::Ready(()),
        };

        if t > time {
            return SourcePollOk::Scheduled((), t)
        }

        let val = if vec.len() == 1 {
            let (_, mut vec) = self.0.pop_first().unwrap();
            vec.pop_front().unwrap()
        } else {
            let vec = self.0.get_mut(&t).unwrap();
            vec.pop_front().unwrap()
        };

        match val {
            OutputType::Event(e) => SourcePollOk::Event(e, t),
            OutputType::Finalize => SourcePollOk::Finalize(t),
            OutputType::Rollback => SourcePollOk::Rollback(t),
        }
    }

    pub fn handle_input_event(&mut self, time: T::Time) {
        todo!()
    }

    pub fn handle_input_rollback(&mut self, time: T::Time) {
        todo!()
    }

    pub fn handle_output_event(&mut self, time: T::Time, event: T::Output) {
        self.insert(time, OutputType::Event(event));
    }
    pub fn handle_output_rollback(&mut self, time: T::Time) {
        self.0.split_off(&time);

        self.insert(time, OutputType::Rollback);
    }

    pub fn handle_output_finalize(&mut self, time: T::Time) {
        self.insert(time, OutputType::Finalize);
    }

    pub fn first_event_time(&self) -> Option<T::Time> {
        self.0.first_key_value().map(|(&t, _)| t)
    }

    fn insert(&mut self, time: T::Time, val: OutputType<T>) {
        match self.0.get_mut(&time) {
            Some(vec) => vec.push_back(val),
            None => {
                let mut vec = VecDeque::new();
                vec.push_back(val);
                self.0.insert(time, vec);
            },
        }
    }
}
