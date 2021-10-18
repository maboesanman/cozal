use core::cmp::Ordering;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct EngineTime<T: Ord + Copy + Default> {
    inner: Arc<RwLock<EngineTimeInner<T>>>,
}

// this is the time that the internal engine can take on.
#[derive(Clone)]
enum EngineTimeInner<T: Ord + Copy + Default> {
    Dead(usize),
    Init,
    Input(T),
    Schedule(EngineTimeSchedule<T>),
}

impl<T: Ord + Copy + Default> EngineTime<T> {
    pub fn new_init() -> Self {
        EngineTime {
            inner: Arc::new(RwLock::new(EngineTimeInner::Init)),
        }
    }

    pub fn raw_time(&self) -> Result<T, usize> {
        let inner: &EngineTimeInner<T> = &self.inner.read().unwrap();
        match inner {
            EngineTimeInner::Dead(id) => Err(*id),
            EngineTimeInner::Init => Ok(T::default()),
            EngineTimeInner::Input(time) => Ok(*time),
            EngineTimeInner::Schedule(inner) => Ok(inner.time),
        }
    }
}

impl<T: Ord + Copy + Default> Ord for EngineTime<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        let inner_self: &EngineTimeInner<T> = &self.inner.read().unwrap();
        let inner_other: &EngineTimeInner<T> = &other.inner.read().unwrap();
        match (inner_self, inner_other) {
            // dead always sorts first, then by id.
            (EngineTimeInner::Dead(s), EngineTimeInner::Dead(o)) => s.cmp(&o),
            (EngineTimeInner::Dead(_), _) => Ordering::Less,
            (_, EngineTimeInner::Dead(_)) => Ordering::Greater,

            // next is init, which sorts before input and scheduled.
            (EngineTimeInner::Init, EngineTimeInner::Init) => Ordering::Equal,
            (EngineTimeInner::Init, _) => Ordering::Less,
            (_, EngineTimeInner::Init) => Ordering::Greater,

            // direct comparisons just pass through.
            (EngineTimeInner::Input(s), EngineTimeInner::Input(o)) => s.cmp(&o),
            (EngineTimeInner::Schedule(s), EngineTimeInner::Schedule(o)) => s.cmp(&o),

            // cross comparisons sort by time, then input before schedule.
            (EngineTimeInner::Input(s), EngineTimeInner::Schedule(o)) => match s.cmp(&o.time) {
                Ordering::Equal => Ordering::Less,
                ord => ord,
            },
            (EngineTimeInner::Schedule(s), EngineTimeInner::Input(o)) => match s.time.cmp(&o) {
                Ordering::Equal => Ordering::Greater,
                ord => ord,
            },
        }
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
        let inner_self: &EngineTimeInner<T> = &self.inner.read().unwrap();
        let inner_other: &EngineTimeInner<T> = &other.inner.read().unwrap();
        match (inner_self, inner_other) {
            (EngineTimeInner::Dead(s), EngineTimeInner::Dead(o)) => s == o,
            (EngineTimeInner::Init, EngineTimeInner::Init) => true,
            (EngineTimeInner::Input(s), EngineTimeInner::Input(o)) => s == o,
            (EngineTimeInner::Schedule(s), EngineTimeInner::Schedule(o)) => {
                s.time == o.time && s.parent_index == o.parent_index && s.parent == o.parent
            },
            _ => false,
        }
    }
}

// impl<T: Ord + Copy + Default> From<EngineTimeSchedule<T>> for EngineTime<T> {
//     fn from(time: EngineTimeSchedule<T>) -> Self {
//         EngineTime::Schedule(time)
//     }
// }

#[derive(Clone)]
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
