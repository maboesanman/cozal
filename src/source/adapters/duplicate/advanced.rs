use std::collections::{BTreeMap, HashMap};

pub struct Advanced<T: Ord + Copy> {
    time_count: BTreeMap<Option<T>, usize>,
    index_map:  HashMap<usize, Option<T>>,
}

impl<T: Ord + Copy> Advanced<T> {
    pub fn new() -> Self {
        Self {
            time_count: BTreeMap::new(),
            index_map:  HashMap::new(),
        }
    }

    pub fn register_new_duplicate(&mut self, index: usize) -> Option<T> {
        let advanced = self.current_aggregate_advancement();

        self.increment_time(advanced);
        self.index_map.insert(index, advanced);

        advanced
    }

    pub fn advance(&mut self, time: T, index: usize) -> Option<T> {
        let prev_time = *self.index_map.get(&index).unwrap();

        if let Some(prev_time) = prev_time {
            if prev_time >= time {
                return None
            }
        }

        let before = self.current_aggregate_advancement();

        self.decrement_time(prev_time);
        self.increment_time(Some(time));

        let after = self.current_aggregate_advancement().unwrap();

        match before {
            None => Some(after),
            Some(before) => {
                if before < after {
                    Some(after)
                } else {
                    None
                }
            },
        }
    }

    fn current_aggregate_advancement(&self) -> Option<T> {
        self.time_count.first_key_value().and_then(|(&k, _)| k)
    }

    fn increment_time(&mut self, time: Option<T>) {
        match self.time_count.get_mut(&time) {
            Some(t) => *t += 1,
            None => {
                self.time_count.insert(time, 1);
            },
        }
    }

    fn decrement_time(&mut self, time: Option<T>) {
        match self.time_count.get_mut(&time) {
            Some(0 | 1) => {
                self.time_count.remove(&time);
            },
            Some(t) => *t -= 1,
            None => {},
        }
    }
}
