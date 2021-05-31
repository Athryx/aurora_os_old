#![no_std]

#![feature(lang_items)]
#![feature(asm)]

pub mod ext;
pub mod os;

mod uses;
mod rt;
mod panicking;

use uses::*;

pub use core::*;
