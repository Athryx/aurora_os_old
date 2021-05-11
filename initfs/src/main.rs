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
			"mov al, 0x41",
			"mov dx, 0xe9",
			"lbl:",
			"out dx, al",
			"jmp lbl",
			options (noreturn));
	}
}
