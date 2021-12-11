#![warn(unused_features)]
#![feature(map_first_last)]
#![feature(maybe_uninit_extra)]
#![feature(new_uninit)]
#![feature(hash_drain_filter)]
#![feature(generic_associated_types)]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(clippy::or_fun_call)]
#![feature(once_cell)]
// TODO: remove this allow
// #![allow(dead_code)]
#![allow(clippy::module_inception)]

pub mod source;
pub mod transposer;
pub(self) mod util;
