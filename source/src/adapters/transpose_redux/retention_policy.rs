pub struct RetentionPolicy<T: Ord + Copy> {
    source_last_finalized: T,
    caller_last_advanced:  T,
}

impl<T: Ord + Copy> RetentionPolicy<T> {
    pub fn new(initial: T) -> Self {
        Self {
            source_last_finalized: initial,
            caller_last_advanced:  initial,
        }
    }

    pub fn get_retain_after(&self) -> T {
        std::cmp::min(self.source_last_finalized, self.caller_last_advanced)
    }

    pub fn source_finalize(&mut self, time: T) -> bool {
        debug_assert!(time >= self.source_last_finalized);

        let old = self.get_retain_after();
        self.source_last_finalized = time;
        let new = self.get_retain_after();

        old != new
    }

    pub fn caller_advance(&mut self, time: T) -> bool {
        debug_assert!(time >= self.caller_last_advanced);

        let old = self.get_retain_after();
        self.caller_last_advanced = time;
        let new = self.get_retain_after();

        old != new
    }
}
