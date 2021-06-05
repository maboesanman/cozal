use std::sync::RwLock;

use matches;
use pin_project::pin_project;

use crate::core::Transposer;

use super::engine_time::EngineTime;

#[pin_project]
pub struct UpdateItem<'a, T: Transposer> {
    #[pin]
    pub time: EngineTime<'a, T::Time>,
    // TODO: EngineTime and UpdateItemData both track the same thing. they probably should be merged.
    pub data: UpdateItemData<T>,
    data_emitted: RwLock<DataEmitted<T::Time>>,
}

impl<'a, T: Transposer> UpdateItem<'a, T> {
    pub fn new(time: EngineTime<'a, T::Time>, data: UpdateItemData<T>) -> Self {
        UpdateItem {
            time,
            data,
            data_emitted: RwLock::new(DataEmitted::Pending),
        }
    }

    pub fn mark_none_emitted(&self) {
        let data_emitted = self.data_emitted.read().unwrap();
        debug_assert!(matches!(*data_emitted, DataEmitted::Pending));

        *self.data_emitted.write().unwrap() = DataEmitted::None
    }
    pub fn mark_event_emitted(&self) {
        let data_emitted = self.data_emitted.read().unwrap();
        debug_assert!(matches!(*data_emitted, DataEmitted::Pending));

        *self.data_emitted.write().unwrap() = DataEmitted::Event
    }

    pub fn mark_state_emitted(&self, time: T::Time) {
        debug_assert!(self.time.raw_time() <= time);

        match *self.data_emitted.read().unwrap() {
            DataEmitted::Event => return,
            DataEmitted::State(t) => {
                if time < t {
                    *self.data_emitted.write().unwrap() = DataEmitted::State(time)
                }
            }
            _ => *self.data_emitted.write().unwrap() = DataEmitted::Event,
        }
    }

    pub fn data_emitted(&self) -> DataEmitted<T::Time> {
        *self.data_emitted.read().unwrap()
    }

    pub fn events_emitted(&self) -> bool {
        matches!(self.data_emitted(), DataEmitted::Event)
    }
}

pub enum UpdateItemData<T: Transposer> {
    Init(Box<T>),
    Input(Box<[T::Input]>),
    Schedule,
}

#[derive(Clone, Copy)]
pub enum DataEmitted<T: Ord + Copy> {
    Event,
    State(T),
    None,
    Pending,
}

impl<T: Ord + Copy> DataEmitted<T> {
    pub fn any(&self) -> bool {
        match self {
            Self::Event => true,
            Self::State(_) => true,
            Self::None => false,
            Self::Pending => false,
        }
    }
    pub fn done(&self) -> bool {
        !matches!(self, Self::Pending)
    }
}
