//! Basic library that has code that is shared between userspace and kernel
#![no_std]

#![feature(asm)]
#![feature(const_fn_trait_bound)]

pub mod atomic;
pub mod cell;
pub mod futex;
pub mod misc;
pub mod ptr;

mod uses;
