#![feature(asm)]

mod uses;

use uses::*;

fn main ()
{
	unsafe
	{
		asm!(
			"lbl:",
			"mov rsi, 0",
			"mov rdx, 53",
			"syscall",
			"out 0xe9, al",
			"jmp lbl",
			options (noreturn));
	}
}
