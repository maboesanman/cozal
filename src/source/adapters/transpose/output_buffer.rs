use std::collections::{BTreeMap, VecDeque};

use crate::source::source_poll::SourcePollOk;
use crate::transposer::Transposer;

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
            unsafe {
                let (_, mut vec) = self.0.pop_first().unwrap_unchecked();
                vec.pop_front().unwrap_unchecked()
            }
        } else {
            unsafe {
                let vec = self.0.get_mut(&t).unwrap_unchecked();
                vec.pop_front().unwrap_unchecked()
            }
        };

        return match val {
            OutputType::Event(e) => SourcePollOk::Event(e, t),
            OutputType::Finalize => SourcePollOk::Finalize(t),
            OutputType::Rollback => SourcePollOk::Rollback(t),
        }
    }
    pub fn handle_event(&mut self, time: T::Time, event: T::Output) {
        self.insert(time, OutputType::Event(event));
    }
    pub fn handle_rollback(&mut self, time: T::Time) {
        self.0.split_off(&time);

        self.insert(time, OutputType::Rollback);
    }

    pub fn handle_finalize(&mut self, time: T::Time) {
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
