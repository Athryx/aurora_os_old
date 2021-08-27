#![feature(asm)]

use std::os::epoch::sys::{realloc, ReallocOptions, print_debug};

mod uses;

use uses::*;

fn main ()
{
	unsafe
	{
		loop
		{
			//println! ("Hello, World!");
			println! ("dfweoifuwFPUWEPFIOUWEFIUWOIPFUSIPOCUOPFIUPOCVUOSPCUOISFUOIWSFUCOPISFUOPIfupasoufioweufopivuoiwevuawivuopsupoievuoisvsdvsdvkdljskvjdfv");
		}

		let options = ReallocOptions::READ | ReallocOptions::WRITE | ReallocOptions::EXEC;
		let (addr, size) = realloc (0, 4096, 0, options).unwrap ();
		let (addr2, size2) = realloc (0, 4 * 4096, 0x47000, options).unwrap ();
		let _ = realloc (addr2, 0, 0, options).unwrap ();
		let (addr, size) = realloc (addr, 2 * 4096, 0, options).unwrap ();
		let (addr, size) = realloc (addr, 4 * 4096, 0x46000, options).unwrap ();
		let (addr, size) = realloc (addr, 5 * 4096, 0x46000, options).unwrap ();

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
