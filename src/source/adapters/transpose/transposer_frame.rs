use core::cmp::Ordering;

use super::super::{
    context::{ExpireEventError, ScheduleEventError},
    ExpireHandle, Transposer,
};

use super::{engine_time::EngineTime, update_item::UpdateItem};
use super::{
    engine_time::EngineTimeSchedule, expire_handle_factory::ExpireHandleFactory,
    input_buffer::InputBuffer,
};

use im::{HashMap, OrdMap};
use rand::SeedableRng;
use rand_chacha::{rand_core::block::BlockRng, ChaCha12Core};

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
    T::Scheduled: Clone,
{
    pub fn new(transposer: T, init_time: &'a EngineTime<'a, T::Time>, rng_seed: [u8; 32]) -> Self {
        Self {
            transposer,
            internal: TransposerFrameInternal::new(init_time, rng_seed),
        }
    }

    pub fn init_next(
        &mut self,
        update_item: &'a UpdateItem<'a, T>,
    ) -> Option<(EngineTimeSchedule<'a, T::Time>, T::Scheduled)> {
        self.internal.current_time = &update_item.time;
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
        let next_input_time = next_input_time.map(EngineTime::Input);

        let next_schedule_time = self.internal.schedule.get_min().map(|(k, _)| k);
        let next_schedule_time = next_schedule_time.map(|time| EngineTime::Schedule(time.clone()));

        match (next_input_time, next_schedule_time) {
            (None, None) => None,
            (None, Some(time)) => Some(time),
            (Some(time), None) => Some(time),
            (Some(input), Some(schedule)) => match input.cmp(&schedule) {
                Ordering::Less => Some(input),
                Ordering::Greater => Some(schedule),
                Ordering::Equal => unreachable!(),
            },
        }
    }
}

#[derive(Clone)]
pub struct TransposerFrameInternal<'a, T: Transposer>
where
    T::Scheduled: Clone,
{
    pub current_time: &'a EngineTime<'a, T::Time>,
    pub scheduling_index: usize,
    // schedule and expire_handles
    pub schedule: OrdMap<EngineTimeSchedule<'a, T::Time>, T::Scheduled>,
    pub expire_handles: HashMap<ExpireHandle, EngineTimeSchedule<'a, T::Time>>,

    pub expire_handle_factory: ExpireHandleFactory,

    pub rng: BlockRng<ChaCha12Core>,
}

impl<'a, T: Transposer> TransposerFrameInternal<'a, T>
where
    T::Scheduled: Clone,
{
    fn new(time: &'a EngineTime<'a, T::Time>, rng_seed: [u8; 32]) -> Self {
        Self {
            current_time: time,
            scheduling_index: 0,
            schedule: OrdMap::new(),
            expire_handles: HashMap::new(),
            expire_handle_factory: ExpireHandleFactory::new(),
            rng: BlockRng::new(ChaCha12Core::from_seed(rng_seed)),
        }
    }

    pub fn schedule_event(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<(), ScheduleEventError> {
        if time < self.current_time.raw_time() {
            return Err(ScheduleEventError::NewEventBeforeCurrent);
        }

        let time = EngineTimeSchedule {
            time,
            parent: self.current_time,
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
        if time < self.current_time.raw_time() {
            return Err(ScheduleEventError::NewEventBeforeCurrent);
        }

        let handle = self.expire_handle_factory.next();
        let time = EngineTimeSchedule {
            time,
            parent: self.current_time,
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

    fn pop_schedule_event(&mut self) -> Option<(EngineTimeSchedule<'a, T::Time>, T::Scheduled)> {
        let (result, new_schedule) = self.schedule.without_min_with_key();
        self.schedule = new_schedule;

        result
    }
}
