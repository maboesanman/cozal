#![warn(unused_features)]
#![feature(map_first_last)]
#![feature(maybe_uninit_extra)]
#![feature(new_uninit)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(single_use_lifetimes)]
#![allow(clippy::or_fun_call)]
// TODO: remove this allow
#![allow(dead_code)]

#[cfg(test)]
mod test;

pub mod source;
pub mod transposer;
