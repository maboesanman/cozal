#![warn(unused_features)]
#![feature(map_first_last)]
#![feature(generic_associated_types)]
#![feature(once_cell)]
#![deny(unsafe_op_in_unsafe_fn)]
// TODO: remove this allow
#![allow(dead_code)]
#![allow(clippy::module_inception)]

pub mod source;
pub mod transposer;
pub(self) mod util;
