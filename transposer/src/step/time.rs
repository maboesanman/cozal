#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SubStepTime<T: Ord + Copy> {
    // the canonical order that this time occured
    pub index: usize,

    // the actual time this occured
    pub time: T,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScheduledTime<T: Ord + Copy> {
    pub time:           T,
    pub parent_index:   usize,
    pub emission_index: usize,
}

impl<T: Ord + Copy> SubStepTime<T> {
    pub fn index(&self) -> usize {
        self.index
    }

    pub fn new_init(time: T) -> Self {
        SubStepTime {
            index: 0,
            time,
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
