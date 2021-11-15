use super::frame_metadata::FrameMetaData;
use super::EngineTimeSchedule;
use crate::transposer::Transposer;

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
