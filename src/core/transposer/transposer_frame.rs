use super::{engine_time::EngineTimeSchedule, expire_handle::{ExpireHandle, ExpireHandleFactory}};
use super::{engine_time::EngineTime, transposer::Transposer};
use im::{HashMap, OrdMap};
use std::sync::{Arc, Weak};

#[derive(Clone)]
pub(super) struct TransposerFrame<T: Transposer> {
    pub transposer: T,
    pub internal: TransposerFrameInternal<T>,
}

impl<T: Transposer> TransposerFrame<T> {
    pub fn new(transposer: T) -> Self {
        Self {
            transposer,
            internal: TransposerFrameInternal::new(),
        }
    }

    pub fn time(&self) -> Arc<EngineTime<T::Time>> {
        self.internal.get_time()
    }

    pub fn get_next_schedule_time(&self) -> Option<T::Time> {
        self.internal.get_next_schedule_time()
    }
}

#[derive(Clone)]
pub(super) struct TransposerFrameInternal<T: Transposer> {
    pub current_time: Arc<EngineTime<T::Time>>,
    pub scheduling_index: usize,
    // schedule and expire_handles
    pub schedule: OrdMap<Arc<EngineTimeSchedule<T::Time>>, T::Scheduled>,
    pub expire_handles: HashMap<ExpireHandle, Weak<EngineTimeSchedule<T::Time>>>,

    pub expire_handle_factory: ExpireHandleFactory,

    // todo add rng seed info
}

impl<T: Transposer> TransposerFrameInternal<T> {
    pub fn new() -> Self {
        Self {
            current_time: EngineTime::new_init(),
            scheduling_index: 0,
            schedule: OrdMap::new(),
            expire_handles: HashMap::new(),
            expire_handle_factory: ExpireHandleFactory::new(),
        }
    }

    pub fn set_time(&mut self, new_time: Arc<EngineTime<T::Time>>) {
        if new_time <= self.current_time {
            panic!()
        } else {
            self.current_time = new_time;
        }
    }

    pub fn get_time(&self) -> Arc<EngineTime<T::Time>> {
        self.current_time.clone()
    }

    pub fn schedule_event(&mut self, time: T::Time, payload: T::Scheduled) -> Result<(), &str> {
        if time < self.get_time().time() {
            return Err("new event cannot sort before current event");
        }

        let time = EngineTimeSchedule {
            time,
            parent: self.current_time.clone(),
            parent_index: self.scheduling_index
        };
        let time = Arc::new(time);

        self.schedule.insert(time, payload);
        self.scheduling_index += 1;

        Ok(())
    }

    pub fn schedule_event_expireable(
        &mut self,
        time: T::Time,
        payload: T::Scheduled,
    ) -> Result<ExpireHandle, &str> {
        if time < self.get_time().time() {
            return Err("new event cannot sort before current event");
        }

        let handle = self.expire_handle_factory.next();
        let time = EngineTimeSchedule {
            time,
            parent: self.current_time.clone(),
            parent_index: self.scheduling_index
        };
        let time = Arc::new(time);

        self.expire_handles.insert(handle, Arc::downgrade(&time));
        self.schedule.insert(time, payload);
        self.scheduling_index += 1;

        Ok(handle)
    }

    pub fn expire_event(&mut self, handle: ExpireHandle) -> Result<(T::Time, T::Scheduled), &str> {
        match self.expire_handles.get(&handle) {
            Some(time) => match time.upgrade() {
                Some(time) => match self.schedule.remove(&time) {
                    Some(payload) => Ok((time.time, payload)),
                    None => Err("expired event"),
                },
                None => Err("expired event"),
            },
            None => Err("invalid handle"),
        }
    }

    pub fn get_next_schedule_time(&self) -> Option<T::Time> {
        self.schedule.get_min().map(|(next, _)| next.time)
    }
}
