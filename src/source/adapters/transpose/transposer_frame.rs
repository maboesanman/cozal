use im::{HashMap, OrdMap};
use rand::SeedableRng;
use rand_chacha::rand_core::block::BlockRng;
use rand_chacha::ChaCha12Core;

use super::engine_time::{EngineTime, EngineTimeSchedule};
use super::expire_handle_factory::ExpireHandleFactory;
use crate::transposer::context::{ExpireEventError, ScheduleEventError};
use crate::transposer::{ExpireHandle, Transposer};

#[derive(Clone)]
pub struct TransposerFrame<T: Transposer>
where
    T::Scheduled: Clone,
{
    pub transposer: T,
    pub internal:   TransposerFrameInternal<T>,
}

impl<T: Transposer> TransposerFrame<T>
where
    T::Scheduled: Clone,
{
    pub fn new(transposer: T, rng_seed: [u8; 32]) -> Self {
        Self {
            transposer,
            internal: TransposerFrameInternal::new(EngineTime::new_init(), rng_seed),
        }
    }

    pub fn get_next_scheduled_time(&self) -> Option<&EngineTimeSchedule<T::Time>> {
        self.internal.schedule.get_min().map(|(k, _)| k)
    }

    pub fn pop_schedule_event(&mut self) -> Option<(EngineTimeSchedule<T::Time>, T::Scheduled)> {
        if let (Some((k, v)), new) = self.internal.schedule.without_min_with_key() {
            self.internal.schedule = new;

            Some((k, v))
        } else {
            None
        }
    }
}

#[derive(Clone)]
pub struct TransposerFrameInternal<T: Transposer>
where
    T::Scheduled: Clone,
{
    pub current_time:     EngineTime<T::Time>,
    pub scheduling_index: usize,

    pub schedule:       OrdMap<EngineTimeSchedule<T::Time>, T::Scheduled>,
    pub expire_handles: HashMap<ExpireHandle, EngineTimeSchedule<T::Time>>,

    pub expire_handle_factory: ExpireHandleFactory,

    pub rng: BlockRng<ChaCha12Core>,
}

impl<T: Transposer> TransposerFrameInternal<T>
where
    T::Scheduled: Clone,
{
    fn new(time: EngineTime<T::Time>, rng_seed: [u8; 32]) -> Self {
        Self {
            current_time:          time,
            scheduling_index:      0,
            schedule:              OrdMap::new(),
            expire_handles:        HashMap::new(),
            expire_handle_factory: ExpireHandleFactory::new(),
            rng:                   BlockRng::new(ChaCha12Core::from_seed(rng_seed)),
        }
    }

    pub fn schedule_event(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<(), ScheduleEventError> {
        if time < self.current_time.raw_time().unwrap() {
            return Err(ScheduleEventError::NewEventBeforeCurrent)
        }

        let time = EngineTimeSchedule {
            time,
            parent: self.current_time.clone(),
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
        if time < self.current_time.raw_time().unwrap() {
            return Err(ScheduleEventError::NewEventBeforeCurrent)
        }

        let handle = self.expire_handle_factory.next();
        let time = EngineTimeSchedule {
            time,
            parent: self.current_time.clone(),
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

    fn pop_schedule_event(&mut self) -> Option<(EngineTimeSchedule<T::Time>, T::Scheduled)> {
        let (result, new_schedule) = self.schedule.without_min_with_key();
        self.schedule = new_schedule;

        result
    }
}
