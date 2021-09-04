#![feature(asm)]

use std::os::epoch::sys::{print_debug, realloc, ReallocOptions};

mod uses;

use uses::*;

fn main()
{
	loop {
		println!("hi");
	}
}
