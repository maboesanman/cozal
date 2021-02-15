#![feature(wake_trait)]
#![feature(map_first_last)]
#![feature(binary_heap_retain)]
#![feature(btree_retain)]
#![feature(btree_drain_filter)]
#![feature(maybe_uninit_ref)]
#![feature(maybe_uninit_extra)]
#![feature(int_bits_const)]
#![feature(new_uninit)]

#[cfg(test)]
mod test;

pub mod core;
pub mod utilities;
