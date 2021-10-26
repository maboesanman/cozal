use im::{HashMap, OrdMap};
use rand::SeedableRng;
use rand_chacha::rand_core::block::BlockRng;
use rand_chacha::ChaCha12Core;

use super::engine_time::{EngineTime, EngineTimeSchedule};
use super::expire_handle_factory::ExpireHandleFactory;
use crate::transposer::context::{ExpireEventError, ScheduleEventError};
use crate::transposer::{ExpireHandle, Transposer};

#[derive(Clone)]
pub struct Frame<T: Transposer>
where
    T::Scheduled: Clone,
{
    pub transposer: T,
    pub internal:   TransposerFrameInternal<T>,
}

impl<T: Transposer> Frame<T>
where
    T::Scheduled: Clone,
{
    pub fn new(transposer: T, rng_seed: [u8; 32]) -> Self {
        Self {
            transposer,
            internal: TransposerFrameInternal::new(rng_seed),
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
    pub schedule:       OrdMap<EngineTimeSchedule<T::Time>, T::Scheduled>,
    pub expire_handles: HashMap<ExpireHandle, EngineTimeSchedule<T::Time>>,

    pub expire_handle_factory: ExpireHandleFactory,

    pub rng: BlockRng<ChaCha12Core>,
}

impl<T: Transposer> TransposerFrameInternal<T>
where
    T::Scheduled: Clone,
{
    fn new(rng_seed: [u8; 32]) -> Self {
        Self {
            schedule:              OrdMap::new(),
            expire_handles:        HashMap::new(),
            expire_handle_factory: ExpireHandleFactory::new(),
            rng:                   BlockRng::new(ChaCha12Core::from_seed(rng_seed)),
        }
    }

    pub fn schedule_event(&mut self, time: EngineTimeSchedule<T::Time>, payload: T::Scheduled) {
        self.schedule.insert(time, payload);
    }

    pub fn schedule_event_expireable(
        &mut self,
        time: EngineTimeSchedule<T::Time>,
        payload: T::Scheduled,
    ) -> ExpireHandle {
        let handle = self.expire_handle_factory.next();
        self.expire_handles.insert(handle, time.clone());
        self.schedule.insert(time, payload);

        handle
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
