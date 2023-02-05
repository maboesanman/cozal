use archery::SharedPointerKind;
use rand::SeedableRng;
use rand_chacha::rand_core::block::BlockRng;
use rand_chacha::ChaCha12Core;
use rpds::{HashTrieMap, RedBlackTreeMap};

use super::expire_handle_factory::ExpireHandleFactory;
use super::time::{ScheduledTime, SubStepTime};
use crate::context::ExpireEventError;
use crate::expire_handle::ExpireHandle;
use crate::Transposer;

#[derive(Clone)]
pub struct TransposerMetaData<T: Transposer, P: SharedPointerKind> {
    pub last_updated: SubStepTime<T::Time>,

    pub schedule: RedBlackTreeMap<ScheduledTime<T::Time>, T::Scheduled, P>,

    pub expire_handles_forward:  HashTrieMap<ExpireHandle, ScheduledTime<T::Time>, P>,
    pub expire_handles_backward: RedBlackTreeMap<ScheduledTime<T::Time>, ExpireHandle, P>,

    pub expire_handle_factory: ExpireHandleFactory,

    pub rng: BlockRng<ChaCha12Core>,
}

impl<T: Transposer, P: SharedPointerKind> TransposerMetaData<T, P> {
    pub fn new(rng_seed: [u8; 32]) -> Self {
        Self {
            last_updated:            SubStepTime::new_init(),
            schedule:                RedBlackTreeMap::new_with_ptr_kind(),
            expire_handles_forward:  HashTrieMap::new_with_hasher_and_ptr_kind(
                std::collections::hash_map::RandomState::default(),
            ),
            expire_handles_backward: RedBlackTreeMap::new_with_ptr_kind(),
            expire_handle_factory:   ExpireHandleFactory::default(),
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
        self.schedule_event(time, payload);

        let handle = self.expire_handle_factory.next();
        self.expire_handles_forward.insert(handle, time);
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
                let payload = self.schedule.remove(time);

                // maps are kept in sync
                let payload = payload.unwrap();
                self.expire_handles_backward.remove(time);
                self.expire_handles_forward.remove(&handle);

                Ok((t, payload))
            },
            None => Err(ExpireEventError::InvalidOrUsedHandle),
        }
    }

    pub fn get_next_scheduled_time(&self) -> Option<&ScheduledTime<T::Time>> {
        self.schedule.first().map(|(k, _)| k)
    }

    pub fn pop_first_event(&mut self) -> Option<(ScheduledTime<T::Time>, T::Scheduled)> {
        let first = match self.schedule.first() {
            Some((ref k, _)) => todo!(),
            None => None,
        };
        if let Some((k, v)) = first {
            if let Some(h) = self.expire_handles_backward.remove(&k) {
                self.expire_handles_forward.remove(&h);
            }

            Some((k, v))
        } else {
            None
        }
    }
}
