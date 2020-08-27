use core::fmt::Debug;
use std::cmp::Ordering;
use std::fmt;

#[derive(Clone)]
pub struct Event<T: Copy + Ord, P> {
    pub timestamp: T,
    pub payload: P,
}

impl<T: Debug + Copy + Ord, P: Debug + Clone> Debug for Event<T, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Point")
            .field("timestamp", &self.timestamp)
            .field("payload", &self.payload)
            .finish()
    }
}

impl<T: Copy + Ord, P: Clone> Ord for Event<T, P> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.timestamp.cmp(&other.timestamp)
    }
}

impl<T: Copy + Ord, P: Clone> PartialOrd for Event<T, P> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Copy + Ord, P: Clone> Eq for Event<T, P> {}

impl<T: Copy + Ord, P: Clone> PartialEq for Event<T, P> {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp.eq(&other.timestamp)
    }
}

#[derive(Clone, Debug)]
pub enum RollbackPayload<P> {
    Payload(P),
    Rollback,
}

// impl<P> Ord for RollbackPayload<P> {
//     fn cmp(&self, other: &Self) -> Ordering {
//         self.timestamp.cmp(&other.timestamp)
//     }
// }

// impl<P> Ord for RollbackPayload<P>
// where P: Ord {
//     fn cmp(&self, other: &Self) -> Option<Ordering> {
//         match (self, other) {
//             (RollbackPayload::Payload(p1), RollbackPayload::Payload(p2)) => p1.cmp(p2),
//             (RollbackPayload::Payload(_), RollbackPayload::Rollback) => Ordering::Greater,
//             (RollbackPayload::Rollback, RollbackPayload::Payload(_)) => Ordering::Less,
//             (RollbackPayload::Rollback, RollbackPayload::Rollback) => Ordering::Equal,
//         }
//     }
// }

// impl<P> PartialOrd for RollbackPayload<P>
// where P: PartialOrd {
//     fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
//         match (self, other) {
//             (RollbackPayload::Payload(p1), RollbackPayload::Payload(p2)) => p1.partial_cmp(p2),
//             (RollbackPayload::Payload(_), RollbackPayload::Rollback) => Some(Ordering::Greater),
//             (RollbackPayload::Rollback, RollbackPayload::Payload(_)) => Some(Ordering::Less),
//             (RollbackPayload::Rollback, RollbackPayload::Rollback) => Some(Ordering::Equal),
//         }
//     }
// }

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

// impl<P> Eq for RollbackPayload<P> {}

// impl<P> PartialEq for RollbackPayload<P>
// where P: PartialEq {
//     fn eq(&self, other: &Self) -> bool {
//         match (self, other) {
//             (RollbackPayload::Payload(p1), RollbackPayload::Payload(p2)) => p1 == p2,
//             (RollbackPayload::Payload(_), RollbackPayload::Rollback) => false,
//             (RollbackPayload::Rollback, RollbackPayload::Payload(_)) => false,
//             (RollbackPayload::Rollback, RollbackPayload::Rollback) => true,
//         }
//     }
// }

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
