use core::cmp::Ordering;
use std::sync::{Arc, RwLock};

#[derive(Clone, Debug)]
pub struct EngineTime<T: Ord + Copy + Default> {
    index: usize,
    time:  T,
}

impl<T: Ord + Copy + Default> EngineTime<T> {
    pub fn index(&self) -> usize {
        self.index
    }

    pub fn new_init() -> Self {
        EngineTime {
            index: 0,
            time:  T::default(),
        }
    }

    pub fn new_input(index: usize, time: T) -> Self {
        EngineTime {
            index,
            time,
        }
    }

    pub fn new_scheduled(index: usize, time: EngineTimeSchedule<T>) -> Self {
        EngineTime {
            index,
            time: time.time,
        }
    }

    pub fn spawn_scheduled(&self, time: T, parent_index: usize) -> EngineTimeSchedule<T> {
        EngineTimeSchedule {
            time,
            parent_index: self.index(),
            emission_index: parent_index,
        }
    }

    pub fn raw_time(&self) -> T {
        self.time
    }
}

impl<T: Ord + Copy + Default> Ord for EngineTime<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.index.cmp(&other.index)
    }
}

impl<T: Ord + Copy + Default> PartialOrd for EngineTime<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Ord + Copy + Default> Eq for EngineTime<T> {}

impl<T: Ord + Copy + Default> PartialEq for EngineTime<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

#[derive(Clone, Debug)]
pub struct EngineTimeSchedule<T: Ord + Copy + Default> {
    pub time:           T,
    pub parent_index:   usize,
    pub emission_index: usize,
}

impl<T: Ord + Copy + Default> Ord for EngineTimeSchedule<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.time.cmp(&other.time) {
            Ordering::Equal => match self.parent_index.cmp(&other.parent_index) {
                Ordering::Equal => self.emission_index.cmp(&other.emission_index),
                ord => ord,
            },
            ord => ord,
        }
    }
}

impl<T: Ord + Copy + Default> PartialOrd for EngineTimeSchedule<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Ord + Copy + Default> Eq for EngineTimeSchedule<T> {}

impl<T: Ord + Copy + Default> PartialEq for EngineTimeSchedule<T> {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
            && self.emission_index == other.emission_index
            && self.parent_index == other.parent_index
    }
}
