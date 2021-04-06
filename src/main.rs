#![no_std]
#![no_main]

#![feature(asm)]

mod arch;
mod int;
mod util;
mod sched;
mod time;

mod uses;
mod gdt;

use core::panic::PanicInfo;
use arch::x64::*;
use sched::Regs;
use int::*;
use int::idt::Handler;

#[panic_handler]
fn panic(info: &PanicInfo) -> !
{
	println! ("{}", info);

	loop {
		hlt ();
	}
}

fn double_fault (_: &Regs, _: u64) -> Option<&Regs>
{
	println! ("double fault");
	None
}

fn gp_exception (_: &Regs, _: u64) -> Option<&Regs>
{
	println! ("general protection exception");
	None
}

fn page_fault (regs: &Regs, code: u64) -> Option<&Regs>
{
	let ring = if code & idt::PAGE_FAULT_USER != 0
	{
		"user"
	}
	else
	{
		"kernel"
	};

	let action = if code & idt::PAGE_FAULT_EXECUTE != 0
	{
		"instruction fetch"
	}
	else
	{
		if code & idt::PAGE_FAULT_WRITE != 0
		{
			"write"
		}
		else
		{
			"read"
		}
	};


	panic! (r"page fault accessing virtual address {}
			page fault during {} {}
			non present page: {}
			reserved bit set: {}
			registers:
			{:?}",
			get_cr2 (),
			ring, action,
			code & idt::PAGE_FAULT_PROTECTION == 0,
			code & idt::PAGE_FAULT_RESERVED != 0,
			regs);
}

fn init () -> Result<(), util::Err>
{
	gdt::init ();

	pic::remap (idt::PICM_OFFSET, idt::PICS_OFFSET);
	idt::init ();

	Handler::First(page_fault).register (idt::EXC_PAGE_FAULT)?;
	Handler::Normal(double_fault).register (idt::EXC_DOUBLE_FAULT)?;
	Handler::Normal(gp_exception).register (idt::EXC_GENERAL_PROTECTION_FAULT)?;

	time::pit::init ()?;

	Ok(())
}

#[no_mangle]
pub extern "C" fn _start () -> !
{
	init ().expect ("kernel init failed");

	println! ("epoch v0.0.1");

	sti ();

	loop {
		hlt ();
	}
}
