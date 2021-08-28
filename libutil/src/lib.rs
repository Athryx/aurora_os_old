//! Basic library that depends only on rust core library and has code that is shared between userspace and kernel
#![no_std]

#![feature(asm)]

pub mod atomic;
pub mod cell;

mod uses;
