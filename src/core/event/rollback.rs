use super::Event;
use std::cmp::Ordering;

#[derive(Clone, Debug)]
pub enum RollbackPayload<P> {
    Payload(P),
    Rollback,
}

pub type RollbackEvent<T, P> = Event<T, RollbackPayload<P>>;

impl<P> PartialOrd for RollbackPayload<P> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (RollbackPayload::Payload(_), RollbackPayload::Payload(_)) => None,
            (RollbackPayload::Payload(_), RollbackPayload::Rollback) => Some(Ordering::Greater),
            (RollbackPayload::Rollback, RollbackPayload::Payload(_)) => Some(Ordering::Less),
            (RollbackPayload::Rollback, RollbackPayload::Rollback) => Some(Ordering::Equal),
        }
    }
}

impl<P> PartialEq for RollbackPayload<P> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (RollbackPayload::Payload(_), RollbackPayload::Payload(_)) => false,
            (RollbackPayload::Payload(_), RollbackPayload::Rollback) => false,
            (RollbackPayload::Rollback, RollbackPayload::Payload(_)) => false,
            (RollbackPayload::Rollback, RollbackPayload::Rollback) => true,
        }
    }
}
