use std::cmp::Ordering;
use std::sync::Arc;

// this is the time that the internal engine can take on.
pub enum EngineTime<T: Ord + Copy + Default> {
    Init,
    Input(T),
    Schedule(EngineTimeSchedule<T>),
}

impl<T: Ord + Copy + Default> EngineTime<T> {
    pub fn raw_time(&self) -> T {
        match self {
            Self::Init => T::default(),
            Self::Input(time) => *time,
            Self::Schedule(inner) => inner.time,
        }
    }

    pub fn new_init() -> Arc<Self> {
        Arc::new(EngineTime::Init)
    }

    pub fn new_input(time: T) -> Arc<Self> {
        Arc::new(EngineTime::Input(time))
    }

    pub fn new_schedule(time: T, parent: Arc<Self>, parent_index: usize) -> Arc<Self> {
        Arc::new(EngineTime::Schedule(EngineTimeSchedule{
            time,
            parent,
            parent_index,
        }))
    }
}

impl<T: Ord + Copy + Default> Ord for EngineTime<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Init, Self::Init) => Ordering::Equal,
            (Self::Init, Self::Input(_)) => Ordering::Less,
            (Self::Init, Self::Schedule(_)) => Ordering::Less,
            (Self::Input(_), Self::Init) => Ordering::Greater,
            (Self::Input(s), Self::Input(o)) => s.cmp(&o),
            (Self::Input(s), Self::Schedule(o)) => {
                match s.cmp(&o.time) {
                    Ordering::Equal => Ordering::Less,
                    ord => ord,
                }
            }
            (Self::Schedule(_), Self::Init) => Ordering::Greater,
            (Self::Schedule(s), Self::Input(o)) => {
                match s.time.cmp(&o) {
                    Ordering::Equal => Ordering::Greater,
                    ord => ord,
                }
            }
            (Self::Schedule(s), Self::Schedule(o)) => s.cmp(&o),
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
        match (self, other) {
            (Self::Init, Self::Init) => true,
            (Self::Input(s), Self::Input(o)) => s == o,
            (Self::Schedule(s), Self::Schedule(o)) => {
                s.time == o.time && s.parent_index == o.parent_index && s.parent == o.parent
            },
            _ => false,
        }
    }
}

impl<T: Ord + Copy + Default> From<EngineTimeSchedule<T>> for Arc<EngineTime<T>> {
    fn from(time: EngineTimeSchedule<T>) -> Self {
        Arc::new(EngineTime::Schedule(time))
    }
}


#[derive(Clone)]
pub struct EngineTimeSchedule<T: Ord + Copy + Default> {
    pub time: T,
    pub parent: Arc<EngineTime<T>>,
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
        self.time == other.time && self.parent_index == other.parent_index && self.parent == other.parent
    }
}