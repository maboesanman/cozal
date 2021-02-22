use crate::core::{Transposer, transposer::expire_handle::ExpireHandle};

use super::{engine_context::LazyState, engine_time::EngineTimeSchedule, expire_handle_factory::ExpireHandleFactory, transposer_update::TransposerUpdate};
use super::{engine_time::EngineTime};

use im::{HashMap, OrdMap};
use std::{pin::Pin, sync::{Arc, Weak}};

#[derive(Clone)]
pub(super) struct TransposerFrame<T: Transposer>
where T::Scheduled: Clone {
    pub transposer: T,
    pub internal: TransposerFrameInternal<T>,
}

impl<T: Transposer> TransposerFrame<T>
where
    T: Clone, 
    T::Scheduled: Clone {
    pub fn new(transposer: T) -> Self {
        Self {
            transposer,
            internal: TransposerFrameInternal::new(),
        }
    }

    // pub fn prepare_init(
    //     &mut self
    // ) -> WrappedFuture<'_, T> {
    //     self.internal.set_time(EngineTime::new_init());
    //     WrappedFuture::new(self)
    // }

    // pub fn prepare_update(
    //     &mut self,
    //     next_input_time: Option<T::Time>,
    // ) -> PrepareUpdateResult<'_, T> {

    // }
}

pub(super) enum PrepareUpdateResult<'a, T: Transposer> {
    Input{
        update: TransposerUpdate<'a, T>
    },
    Schedule{
        update: TransposerUpdate<'a, T>,
        time: T::Time,
        payload: T::Scheduled,
    },
    None,
}

#[derive(Clone)]
pub(super) struct TransposerFrameInternal<T: Transposer> 
where T::Scheduled: Clone {
    pub current_time: Arc<EngineTime<T::Time>>,
    pub scheduling_index: usize,
    // schedule and expire_handles
    pub schedule: OrdMap<EngineTimeSchedule<T::Time>, T::Scheduled>,
    pub expire_handles: HashMap<ExpireHandle, EngineTimeSchedule<T::Time>>,

    pub expire_handle_factory: ExpireHandleFactory,

    // todo add rng seed info
}

impl<T: Transposer> TransposerFrameInternal<T> 
where T::Scheduled: Clone {
    fn new() -> Self {
        Self {
            current_time: EngineTime::new_init(),
            scheduling_index: 0,
            schedule: OrdMap::new(),
            expire_handles: HashMap::new(),
            expire_handle_factory: ExpireHandleFactory::new(),
        }
    }

    fn set_time(&mut self, new_time: Arc<EngineTime<T::Time>>) {
        if new_time <= self.current_time {
            panic!()
        } else {
            self.current_time = new_time;
        }
    }

    pub fn get_time(&self) -> Arc<EngineTime<T::Time>> {
        self.current_time.clone()
    }

    pub fn schedule_event(&mut self, time: T::Time, payload: T::Scheduled) -> Result<(), &str> {
        if time < self.get_time().raw_time() {
            return Err("new event cannot sort before current event");
        }

        let time = EngineTimeSchedule {
            time,
            parent: self.current_time.clone(),
            parent_index: self.scheduling_index
        };

        self.schedule.insert(time, payload);
        self.scheduling_index += 1;

        Ok(())
    }

    pub fn schedule_event_expireable(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<ExpireHandle, &str> {
        if time < self.get_time().raw_time() {
            return Err("new event cannot sort before current event");
        }

        let handle = self.expire_handle_factory.next();
        let time = EngineTimeSchedule {
            time,
            parent: self.current_time.clone(),
            parent_index: self.scheduling_index
        };

        self.expire_handles.insert(handle, time.clone());
        self.schedule.insert(time, payload);
        self.scheduling_index += 1;

        Ok(handle)
    }

    pub fn expire_event(&mut self, handle: ExpireHandle) -> Result<(T::Time, T::Scheduled), &str> {
        match self.expire_handles.get(&handle) {
            Some(time) => match self.schedule.remove(&time) {
                Some(payload) => Ok((time.time, payload)),
                None => Err("expired event"),
            },
            None => Err("invalid handle"),
        }
    }

    fn get_next_schedule_time(&self) -> Option<&EngineTimeSchedule<T::Time>> {
        self.schedule.get_min().map(|(next, _)| next)
    }

    fn pop_schedule_event(&mut self) -> Option<(EngineTimeSchedule<T::Time>, T::Scheduled)> {
        let (result, new_schedule) = self.schedule.without_min_with_key();
        self.schedule = new_schedule;

        result
    }
}
