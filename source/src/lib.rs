#![allow(incomplete_features)]
#![deny(unsafe_op_in_unsafe_fn)]
// TODO: remove this allow
#![feature(iter_collect_into)]
#![feature(is_sorted)]
#![feature(async_fn_in_trait)]

pub mod source_poll;

pub mod adapters;
pub mod sources;
pub mod traits;

pub use self::source_poll::SourcePoll;
pub use self::traits::Source;
