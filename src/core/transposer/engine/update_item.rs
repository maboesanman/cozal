use std::sync::RwLock;

use pin_project::pin_project;

use crate::core::Transposer;

use super::engine_time::EngineTime;

// pointer needs to be the top argument as its target may have pointers into inputs or transposer.
#[pin_project]
pub struct UpdateItem<'a, T: Transposer> {
    #[pin]
    pub time: EngineTime<'a, T::Time>,
    // TODO: EngineTime and UpdateItemData both track the same thing. they probably should be merged.
    pub data: UpdateItemData<T>,
    pub events_emitted: RwLock<DataEmitted<T::Time>>,
}

pub enum UpdateItemData<T: Transposer> {
    Init(Box<T>),
    Input(Box<[T::Input]>),
    Schedule,
}

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
