use im::{HashMap, OrdMap};
use rand::SeedableRng;
use rand_chacha::rand_core::block::BlockRng;
use rand_chacha::ChaCha12Core;

use super::engine_time::EngineTimeSchedule;
use super::expire_handle_factory::ExpireHandleFactory;
use crate::transposer::context::ExpireEventError;
use crate::transposer::{ExpireHandle, Transposer};

#[cfg(test)]
mod test;

#[derive(Clone)]
pub struct Frame<T: Transposer>
where
    T::Scheduled: Clone,
{
    pub transposer: T,
    pub metadata:   FrameMetaData<T>,
}

impl<T: Transposer> Frame<T>
where
    T::Scheduled: Clone,
{
    pub fn new(transposer: T, rng_seed: [u8; 32]) -> Self {
        Self {
            transposer,
            metadata: FrameMetaData::new(rng_seed),
        }
    }

    pub fn get_next_scheduled_time(&self) -> Option<&EngineTimeSchedule<T::Time>> {
        self.metadata.get_next_scheduled_time()
    }

    pub fn pop_schedule_event(&mut self) -> Option<(EngineTimeSchedule<T::Time>, T::Scheduled)> {
        self.metadata.pop_first_event()
    }
}

#[derive(Clone)]
pub struct FrameMetaData<T: Transposer>
where
    T::Scheduled: Clone,
{
    pub schedule: OrdMap<EngineTimeSchedule<T::Time>, T::Scheduled>,

    pub expire_handles_forward:  HashMap<ExpireHandle, EngineTimeSchedule<T::Time>>,
    pub expire_handles_backward: OrdMap<EngineTimeSchedule<T::Time>, ExpireHandle>,

    pub expire_handle_factory: ExpireHandleFactory,

    pub rng: BlockRng<ChaCha12Core>,
}

impl<T: Transposer> FrameMetaData<T>
where
    T::Scheduled: Clone,
{
    fn new(rng_seed: [u8; 32]) -> Self {
        Self {
            schedule:                OrdMap::new(),
            expire_handles_forward:  HashMap::new(),
            expire_handles_backward: OrdMap::new(),
            expire_handle_factory:   ExpireHandleFactory::new(),
            rng:                     BlockRng::new(ChaCha12Core::from_seed(rng_seed)),
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
        self.schedule_event(time.clone(), payload);

        let handle = self.expire_handle_factory.next();
        self.expire_handles_forward.insert(handle, time.clone());
        self.expire_handles_backward.insert(time, handle);

        handle
    }

    pub fn expire_event(
        &mut self,
        handle: ExpireHandle,
    ) -> Result<(T::Time, T::Scheduled), ExpireEventError> {
        match self.expire_handles_forward.get(&handle) {
            Some(time) => match self.schedule.remove(&time) {
                Some(payload) => {
                    let t = time.time;
                    self.expire_handles_backward.remove(&time);
                    self.expire_handles_forward.remove(&handle);
                    Ok((t, payload))
                },
                None => unreachable!(),
            },
            None => Err(ExpireEventError::InvalidOrUsedHandle),
        }
    }

    pub fn get_next_scheduled_time(&self) -> Option<&EngineTimeSchedule<T::Time>> {
        self.schedule.get_min().map(|(k, _)| k)
    }

    pub fn pop_first_event(&mut self) -> Option<(EngineTimeSchedule<T::Time>, T::Scheduled)> {
        if let (Some((k, v)), new) = self.schedule.without_min_with_key() {
            self.schedule = new;

            if let Some(h) = self.expire_handles_backward.remove(&k) {
                self.expire_handles_forward.remove(&h);
            }

            Some((k, v))
        } else {
            None
        }
    }
}
