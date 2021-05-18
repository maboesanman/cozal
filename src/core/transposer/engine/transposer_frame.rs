use std::mem::MaybeUninit;

use crate::core::{
    transposer::{
        context::{ExpireEventError, ScheduleEventError},
        expire_handle::ExpireHandle,
    },
    Transposer,
};

use super::engine_time::EngineTime;
use super::{
    engine_time::EngineTimeSchedule, expire_handle_factory::ExpireHandleFactory,
    input_buffer::InputBuffer, state_map::UpdateItem,
};

use im::{HashMap, OrdMap};

#[derive(Clone)]
pub struct TransposerFrame<'a, T: Transposer>
where
    T::Scheduled: Clone,
{
    pub transposer: T,
    pub internal: TransposerFrameInternal<'a, T>,
}

impl<'a, T: Transposer> TransposerFrame<'a, T>
where
    T: Clone,
    T::Scheduled: Clone,
{
    pub fn new(transposer: T) -> Self {
        Self {
            transposer,
            internal: TransposerFrameInternal::new(),
        }
    }

    pub fn init_next(
        &mut self,
        update_item: &'a UpdateItem<'a, T>,
    ) -> Option<(EngineTimeSchedule<'a, T::Time>, T::Scheduled)> {
        self.internal.current_time = MaybeUninit::new(&update_item.time);
        match &update_item.time {
            EngineTime::Init => None,
            EngineTime::Input(_time) => None,
            // unwrap and rewrap to assert it is not None.
            EngineTime::Schedule(_time) => Some(self.internal.pop_schedule_event().unwrap()),
        }
    }

    pub fn get_next_time(
        &self,
        input_buffer: &InputBuffer<T::Time, T::Input>,
    ) -> Option<EngineTime<'a, T::Time>> {
        let next_input_time = input_buffer.first_time();
        let next_input_time = next_input_time.map(|time| EngineTime::Input(time.clone()));

        let next_schedule_time = self.internal.schedule.get_min().map(|(k, _)| k);
        let next_schedule_time = next_schedule_time.map(|time| EngineTime::Schedule(time.clone()));

        match (next_input_time, next_schedule_time) {
            (None, None) => None,
            (None, Some(time)) => Some(time),
            (Some(time), None) => Some(time),
            (Some(input), Some(schedule)) => match input.cmp(&schedule) {
                std::cmp::Ordering::Less => Some(input),
                std::cmp::Ordering::Greater => Some(schedule),
                std::cmp::Ordering::Equal => unreachable!(),
            },
        }
    }
}

#[derive(Clone)]
pub struct TransposerFrameInternal<'a, T: Transposer>
where
    T::Scheduled: Clone,
{
    pub current_time: MaybeUninit<&'a EngineTime<'a, T::Time>>,
    pub scheduling_index: usize,
    // schedule and expire_handles
    pub schedule: OrdMap<EngineTimeSchedule<'a, T::Time>, T::Scheduled>,
    pub expire_handles: HashMap<ExpireHandle, EngineTimeSchedule<'a, T::Time>>,

    pub expire_handle_factory: ExpireHandleFactory,
    // todo add rng seed info
}

impl<'a, T: Transposer> TransposerFrameInternal<'a, T>
where
    T::Scheduled: Clone,
{
    fn new() -> Self {
        Self {
            current_time: MaybeUninit::uninit(),
            scheduling_index: 0,
            schedule: OrdMap::new(),
            expire_handles: HashMap::new(),
            expire_handle_factory: ExpireHandleFactory::new(),
        }
    }

    pub fn schedule_event(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<(), ScheduleEventError> {
        let current_time = unsafe { self.current_time.assume_init_ref() };
        if time < current_time.raw_time() {
            return Err(ScheduleEventError::NewEventBeforeCurrent);
        }

        let time = EngineTimeSchedule {
            time,
            parent: current_time,
            parent_index: self.scheduling_index,
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
        let current_time = unsafe { self.current_time.assume_init_ref() };
        if time < current_time.raw_time() {
            return Err(ScheduleEventError::NewEventBeforeCurrent);
        }

        let handle = self.expire_handle_factory.next();
        let time = EngineTimeSchedule {
            time,
            parent: current_time,
            parent_index: self.scheduling_index,
        };

        self.expire_handles.insert(handle, time.clone());
        self.schedule.insert(time, payload);
        self.scheduling_index += 1;

        Ok(handle)
    }

    pub fn expire_event(
        &mut self,
        handle: ExpireHandle,
    ) -> Result<(T::Time, T::Scheduled), ExpireEventError> {
        match self.expire_handles.get(&handle) {
            Some(time) => match self.schedule.remove(&time) {
                Some(payload) => Ok((time.time, payload)),
                None => Err(ExpireEventError::ExpiredEvent),
            },
            None => Err(ExpireEventError::InvalidHandle),
        }
    }

    // fn get_next_schedule_time(&self) -> Option<&EngineTimeSchedule<'a, T::Time>> {
    //     self.schedule.get_min().map(|(next, _)| next)
    // }

    fn pop_schedule_event(&mut self) -> Option<(EngineTimeSchedule<'a, T::Time>, T::Scheduled)> {
        let (result, new_schedule) = self.schedule.without_min_with_key();
        self.schedule = new_schedule;

        result
    }
}
