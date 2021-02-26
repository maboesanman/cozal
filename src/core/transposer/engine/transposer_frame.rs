use crate::core::{Transposer, transposer::{context::{ExpireEventError, ScheduleEventError}, expire_handle::ExpireHandle}};

use super::{ engine_time::EngineTimeSchedule, expire_handle_factory::ExpireHandleFactory, transposer_update::TransposerUpdate};
use super::{engine_time::EngineTime};

use im::{HashMap, OrdMap};

#[derive(Clone)]
pub(super) struct TransposerFrame<'a, T: Transposer>
where T::Scheduled: Clone {
    pub transposer: T,
    pub internal: TransposerFrameInternal<'a, T>,
}

impl<'a, T: Transposer> TransposerFrame<'a, T>
where
    T: Clone, 
    T::Scheduled: Clone {
    pub fn new(transposer: T, current_time: &'a EngineTime<'a, T::Time>) -> Self {
        Self {
            transposer,
            internal: TransposerFrameInternal::new(current_time),
        }
    }

    pub fn get_next_schedule_time(&self) -> Option<&EngineTimeSchedule<T::Time>> {
        self.internal.get_next_schedule_time()
    }
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
pub(super) struct TransposerFrameInternal<'a, T: Transposer> 
where T::Scheduled: Clone {
    pub current_time: &'a EngineTime<'a, T::Time>,
    pub scheduling_index: usize,
    // schedule and expire_handles
    pub schedule: OrdMap<EngineTimeSchedule<'a, T::Time>, T::Scheduled>,
    pub expire_handles: HashMap<ExpireHandle, EngineTimeSchedule<'a, T::Time>>,

    pub expire_handle_factory: ExpireHandleFactory,

    // todo add rng seed info
}

impl<'a, T: Transposer> TransposerFrameInternal<'a, T>
where T::Scheduled: Clone {
    fn new(current_time: &'a EngineTime<'a, T::Time>) -> Self {
        Self {
            current_time,
            scheduling_index: 0,
            schedule: OrdMap::new(),
            expire_handles: HashMap::new(),
            expire_handle_factory: ExpireHandleFactory::new(),
        }
    }

    pub fn advance_time(&mut self, new_time: &'a EngineTime<'a, T::Time>) {
        if new_time <= self.current_time {
            panic!()
        } else {
            self.current_time = new_time;
        }
    }

    pub fn schedule_event(&mut self, time: T::Time, payload: T::Scheduled) -> Result<(), ScheduleEventError> {
        if time < self.current_time.raw_time() {
            return Err(ScheduleEventError::NewEventBeforeCurrent);
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
    ) -> Result<ExpireHandle, ScheduleEventError> {
        if time < self.current_time.raw_time() {
            return Err(ScheduleEventError::NewEventBeforeCurrent);
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

    pub fn expire_event(&mut self, handle: ExpireHandle) -> Result<(T::Time, T::Scheduled), ExpireEventError> {
        match self.expire_handles.get(&handle) {
            Some(time) => match self.schedule.remove(&time) {
                Some(payload) => Ok((time.time, payload)),
                None => Err(ExpireEventError::ExpiredEvent),
            },
            None => Err(ExpireEventError::InvalidHandle),
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
