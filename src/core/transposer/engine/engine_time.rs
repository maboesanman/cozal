use std::cmp::Ordering;

// this is the time that the internal engine can take on.
#[derive(Clone)]
pub enum EngineTime<'a, T: Ord + Copy + Default> {
    Init,
    Input(T),
    Schedule(EngineTimeSchedule<'a, T>),
}

impl<'a, T: Ord + Copy + Default> EngineTime<'a, T> {
    pub fn raw_time(&self) -> T {
        match self {
            Self::Init => T::default(),
            Self::Input(time) => *time,
            Self::Schedule(inner) => inner.time,
        }
    }

    pub fn new_init() -> Self {
        EngineTime::Init
    }

    pub fn new_input(time: T) -> Self {
        EngineTime::Input(time)
    }

    pub fn new_schedule(time: T, parent: &'a Self, parent_index: usize) -> Self {
        EngineTime::Schedule(EngineTimeSchedule{
            time,
            parent,
            parent_index,
        })
    }
}

impl<T: Ord + Copy + Default> Ord for EngineTime<'_, T> {
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

impl<T: Ord + Copy + Default> PartialOrd for EngineTime<'_, T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Ord + Copy + Default> Eq for EngineTime<'_, T> {}

impl<T: Ord + Copy + Default> PartialEq for EngineTime<'_, T> {
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

impl<'a, T: Ord + Copy + Default> From<EngineTimeSchedule<'a, T>> for EngineTime<'a, T> {
    fn from(time: EngineTimeSchedule<'a, T>) -> Self {
        EngineTime::Schedule(time)
    }
}


#[derive(Clone)]
pub struct EngineTimeSchedule<'a, T: Ord + Copy + Default> {
    pub time: T,
    pub parent: &'a EngineTime<'a, T>,
    pub parent_index: usize,
}

impl<T: Ord + Copy + Default> Ord for EngineTimeSchedule<'_, T> {
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

impl<T: Ord + Copy + Default> PartialOrd for EngineTimeSchedule<'_, T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Ord + Copy + Default> Eq for EngineTimeSchedule<'_, T> {}

impl<T: Ord + Copy + Default> PartialEq for EngineTimeSchedule<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time && self.parent_index == other.parent_index && self.parent == other.parent
    }
}