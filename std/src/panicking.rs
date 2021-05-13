use core::panic::PanicInfo;

#[lang = "eh_personality"]
#[no_mangle]
extern fn rust_eh_personality () {}

#[lang = "panic_impl"]
#[no_mangle]
extern fn rust_begin_panic (_info: &PanicInfo) -> !
{
	//println! ("{}", info);
	//eprintln! ("{}", info);
	loop {
	}
}
