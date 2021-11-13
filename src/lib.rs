#![warn(unused_features)]
#![feature(map_first_last)]
#![feature(maybe_uninit_extra)]
#![feature(new_uninit)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(single_use_lifetimes)]

#[cfg(test)]
mod test;

pub mod source;
pub mod transposer;
