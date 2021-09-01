#![feature(asm)]

use std::os::epoch::sys::{realloc, ReallocOptions, print_debug};

mod uses;

use uses::*;

fn main ()
{
	loop
	{
		println! ("hi");
	}
}
