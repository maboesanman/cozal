#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct StepTime<T: Ord + Copy + Default> {
    index: usize,
    time:  T,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScheduledTime<T: Ord + Copy + Default> {
    pub time:           T,
    pub parent_index:   usize,
    pub emission_index: usize,
}

impl<T: Ord + Copy + Default> StepTime<T> {
    pub fn index(&self) -> usize {
        self.index
    }

    pub fn new_init() -> Self {
        StepTime {
            index: 0,
            time:  T::default(),
        }
    }

    pub fn new_input(index: usize, time: T) -> Self {
        StepTime {
            index,
            time,
        }
    }

    pub fn new_scheduled(index: usize, time: ScheduledTime<T>) -> Self {
        StepTime {
            index,
            time: time.time,
        }
    }

    pub fn spawn_scheduled(&self, time: T, emission_index: usize) -> ScheduledTime<T> {
        ScheduledTime {
            time,
            parent_index: self.index(),
            emission_index,
        }
    }

    pub fn raw_time(&self) -> T {
        self.time
    }
}
