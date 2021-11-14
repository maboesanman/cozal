use core::cmp::Ordering;
use std::sync::{Arc, RwLock};

#[derive(Clone, Debug)]
pub struct EngineTime<T: Ord + Copy + Default> {
    inner: Arc<(usize, EngineTimeInner<T>)>,
}

// this is the time that the internal engine can take on.
#[derive(Clone, Debug)]
enum EngineTimeInner<T: Ord + Copy + Default> {
    Init,
    Input(T),
    Schedule(EngineTimeSchedule<T>),
}

impl<T: Ord + Copy + Default> EngineTime<T> {
    fn new(index: usize, inner: EngineTimeInner<T>) -> Self {
        EngineTime {
            inner: Arc::new((index, inner)),
        }
    }

    pub fn index(&self) -> usize {
        self.inner.0
    }

    pub fn new_init() -> Self {
        Self::new(0, EngineTimeInner::Init)
    }

    pub fn new_input(index: usize, time: T) -> Self {
        Self::new(index, EngineTimeInner::Input(time))
    }

    pub fn new_scheduled(index: usize, time: EngineTimeSchedule<T>) -> Self {
        Self::new(index, EngineTimeInner::Schedule(time))
    }

    pub fn spawn_scheduled(&self, time: T, parent_index: usize) -> EngineTimeSchedule<T> {
        EngineTimeSchedule {
            time,
            parent: self.clone(),
            parent_index,
        }
    }

    pub fn raw_time(&self) -> Result<T, usize> {
        match &self.inner.1 {
            EngineTimeInner::Init => Ok(T::default()),
            EngineTimeInner::Input(time) => Ok(*time),
            EngineTimeInner::Schedule(inner) => Ok(inner.time),
        }
    }

    pub fn equals_scheduled(&self, other: &EngineTimeSchedule<T>) -> bool {
        if let EngineTimeInner::Schedule(inner) = &self.inner.1 {
            inner == other
        } else {
            false
        }
    }

    pub fn is_init(&self) -> bool {
        matches!(self.inner.1, EngineTimeInner::Init)
    }
    pub fn is_input(&self) -> bool {
        matches!(self.inner.1, EngineTimeInner::Input(_))
    }
    pub fn is_scheduled(&self) -> bool {
        matches!(self.inner.1, EngineTimeInner::Schedule(_))
    }
}

impl<T: Ord + Copy + Default> Ord for EngineTime<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.inner.0.cmp(&other.inner.0)
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
        self.inner.0 == other.inner.0
    }
}

#[derive(Clone, Debug)]
pub struct EngineTimeSchedule<T: Ord + Copy + Default> {
    pub time:         T,
    pub parent:       EngineTime<T>,
    pub parent_index: usize,
}

impl<T: Ord + Copy + Default> Ord for EngineTimeSchedule<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.time.cmp(&other.time) {
            Ordering::Equal => match self.parent.cmp(&other.parent) {
                Ordering::Equal => self.parent_index.cmp(&other.parent_index),
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
            && self.parent_index == other.parent_index
            && self.parent == other.parent
    }
}
