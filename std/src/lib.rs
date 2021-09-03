#![no_std]
#![feature(lang_items)]
#![feature(asm)]

pub mod ext;
pub mod io;
pub mod os;

mod macros;
mod panicking;
mod rt;
mod uses;

pub use core::*;
