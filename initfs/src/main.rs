#![no_std]
#![no_main]

#![feature(naked_functions)]
#![feature(asm)]

mod uses;

use core::panic::PanicInfo;
use uses::*;

#[panic_handler]
fn panic(info: &PanicInfo) -> !
{
	//println! ("{}", info);
	//eprintln! ("{}", info);

	loop {
	}
}

#[no_mangle]
#[naked]
extern "C" fn start ()
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
