#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct EngineTime<T: Ord + Copy + Default> {
    index: usize,
    time:  T,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct EngineTimeSchedule<T: Ord + Copy + Default> {
    pub time:           T,
    pub parent_index:   usize,
    pub emission_index: usize,
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

    pub fn spawn_scheduled(&self, time: T, emission_index: usize) -> EngineTimeSchedule<T> {
        EngineTimeSchedule {
            time,
            parent_index: self.index(),
            emission_index,
        }
    }

    pub fn raw_time(&self) -> T {
        self.time
    }
}
