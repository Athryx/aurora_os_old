use core::panic::PanicInfo;

use crate::uses::*;

#[lang = "eh_personality"]
#[no_mangle]
extern "C" fn rust_eh_personality() {}

#[lang = "panic_impl"]
#[no_mangle]
extern "C" fn rust_begin_panic(info: &PanicInfo) -> !
{
	//println! ("{}", info);
	//eprintln! ("{}", info);
	loop {
		println!("{}", info);
	}
}
