use im::{HashMap, OrdMap};
use rand::SeedableRng;
use rand_chacha::rand_core::block::BlockRng;
use rand_chacha::ChaCha12Core;

use super::expire_handle_factory::ExpireHandleFactory;
use super::ScheduledTime;
use crate::transposer::context::ExpireEventError;
use crate::transposer::{ExpireHandle, Transposer};

#[derive(Clone)]
pub struct TransposerMetaData<T: Transposer>
where
    T::Scheduled: Clone,
{
    pub last_updated: T::Time,

    pub schedule: OrdMap<ScheduledTime<T::Time>, T::Scheduled>,

    pub expire_handles_forward:  HashMap<ExpireHandle, ScheduledTime<T::Time>>,
    pub expire_handles_backward: OrdMap<ScheduledTime<T::Time>, ExpireHandle>,

    pub expire_handle_factory: ExpireHandleFactory,

    pub rng: BlockRng<ChaCha12Core>,
}

impl<T: Transposer> TransposerMetaData<T>
where
    T::Scheduled: Clone,
{
    pub fn new(rng_seed: [u8; 32]) -> Self {
        Self {
            last_updated:            T::Time::default(),
            schedule:                OrdMap::new(),
            expire_handles_forward:  HashMap::new(),
            expire_handles_backward: OrdMap::new(),
            expire_handle_factory:   ExpireHandleFactory::new(),
            rng:                     BlockRng::new(ChaCha12Core::from_seed(rng_seed)),
        }
    }

    pub fn schedule_event(&mut self, time: ScheduledTime<T::Time>, payload: T::Scheduled) {
        self.schedule.insert(time, payload);
    }

    pub fn schedule_event_expireable(
        &mut self,
        time: ScheduledTime<T::Time>,
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
            Some(time) => {
                let t = time.time;

                // SAFETY: maps are kept in sync
                let payload = self.schedule.remove(time).unwrap();
                self.expire_handles_backward.remove(time);
                self.expire_handles_forward.remove(&handle);

                Ok((t, payload))
            },
            None => Err(ExpireEventError::InvalidOrUsedHandle),
        }
    }

    pub fn get_next_scheduled_time(&self) -> Option<&ScheduledTime<T::Time>> {
        self.schedule.get_min().map(|(k, _)| k)
    }

    pub fn pop_first_event(&mut self) -> Option<(ScheduledTime<T::Time>, T::Scheduled)> {
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
