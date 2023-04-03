#![deny(unsafe_op_in_unsafe_fn)]
// TODO: remove this allow
#![allow(dead_code)]

mod source_poll;

// pub mod adapters;
pub mod sources;
pub mod traits;

pub use self::source_poll::SourcePoll;
pub use self::traits::Source;
