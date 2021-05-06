#![no_std]
#![no_main]

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

fn test ()
{
}
