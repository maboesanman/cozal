// mod duplicate;
// mod offload;
mod multiplex;
// mod transpose;
mod concurrent;
mod transpose;

// pub use self::duplicate::Duplicate;
pub use self::concurrent::MutexSource;
pub use self::multiplex::Multiplex;
// pub use self::offload::{offload, OffloadFuture, OffloadSource};
pub use self::transpose::Transpose;
