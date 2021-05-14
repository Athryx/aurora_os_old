#![feature(asm)]

mod uses;

use uses::*;

fn main ()
{
	unsafe
	{
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
