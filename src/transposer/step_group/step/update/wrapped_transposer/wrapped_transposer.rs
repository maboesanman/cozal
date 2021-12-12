use super::transposer_metadata::TransposerMetaData;
use super::ScheduledTime;
use crate::transposer::Transposer;

#[derive(Clone)]
pub struct WrappedTransposer<T: Transposer> {
    pub transposer: T,
    pub metadata:   TransposerMetaData<T>,
}

impl<T: Transposer> WrappedTransposer<T> {
    pub fn new(transposer: T, rng_seed: [u8; 32]) -> Self {
        Self {
            transposer,
            metadata: TransposerMetaData::new(rng_seed),
        }
    }

    pub fn get_next_scheduled_time(&self) -> Option<&ScheduledTime<T::Time>> {
        self.metadata.get_next_scheduled_time()
    }

    pub fn pop_schedule_event(&mut self) -> Option<(ScheduledTime<T::Time>, T::Scheduled)> {
        self.metadata.pop_first_event()
    }
}
