use core::fmt::{self, Write};

use sys::print_debug;

use crate::uses::*;

static mut PRINTER: Printer = Printer();

struct Printer();

impl Write for Printer
{
	fn write_str(&mut self, s: &str) -> fmt::Result
	{
		const SIZE: usize = 10 * core::mem::size_of::<usize>();
		let mut bytes = [0u8; SIZE];
		let mut i = 0;

		for byte in s.bytes() {
			bytes[i] = byte;
			i += 1;
			if i == SIZE {
				i = 0;
				print_debug(&bytes, SIZE as u32);
			}
		}

		print_debug(&bytes, i as u32);

		Ok(())
	}
}

pub fn _print(args: fmt::Arguments)
{
	unsafe { PRINTER.write_fmt(args).unwrap() }
}
