use std::time::Duration;
use std::time::Instant;

pub trait Timestamp: Ord + Copy {
    type Reference;

    fn get_instant(&self, reference: &Self::Reference) -> Instant;
    fn get_timestamp(instant: &Instant, reference: &Self::Reference) -> Self;
}

impl Timestamp for Instant {
    type Reference = ();

    fn get_instant(&self, _reference: &Self::Reference) -> Instant {
        *self
    }
    fn get_timestamp(instant: &Instant, reference: &Self::Reference) -> Self {
        *instant
    }
}

impl Timestamp for Duration {
    type Reference = Instant;

    fn get_instant(&self, reference: &Self::Reference) -> Instant {
        *reference + *self
    }

    fn get_timestamp(instant: &Instant, reference: &Self::Reference) -> Self {
        *instant - *reference
    }
}