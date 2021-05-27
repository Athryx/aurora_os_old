#![feature(asm)]

use std::os::epoch::sys::{realloc, ReallocOptions};

mod uses;

use uses::*;

fn main ()
{
	unsafe
	{
		let options = ReallocOptions::READ | ReallocOptions::WRITE;
		let (addr, size) = realloc (0, 4096, 0, options).unwrap ();
		asm!(
			"lbl:",
			"mov rax, 0",
			"mov rbx, 53",
			"syscall",
			"mov rax, rbx",
			"out 0xe9, al",
			"mov rax, rdx",
			"out 0xe9, al",
			"jmp lbl",
			options (noreturn));
	}
}
