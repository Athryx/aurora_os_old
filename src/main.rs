#![no_std]
#![no_main]

#![feature(asm)]
#![feature(const_fn)]
#![feature(maybe_uninit_uninit_array)]
#![feature(array_methods)]
#![feature(alloc_error_handler)]
#![feature(try_trait)]

#![allow(non_upper_case_globals)]
#![allow(dead_code)]

extern crate alloc;

mod arch;
mod int;
mod util;
mod sched;
mod mem;
mod time;

mod uses;
mod gdt;
mod kdata;

use uses::*;
use core::panic::PanicInfo;
use bootloader::bootinfo::BootInfo;
use arch::x64::*;
use sched::Regs;
use int::*;
use int::idt::Handler;
use util::misc;
use mem::*;
use alloc::boxed::Box;
use alloc::collections::*;
use alloc::vec;

#[panic_handler]
fn panic(info: &PanicInfo) -> !
{
	println! ("{}", info);
	eprintln! ("{}", info);

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

	// can't indent because it will print tabs
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

fn init (boot_info: &'static BootInfo) -> Result<(), util::Err>
{
	misc::init (boot_info.physical_memory_offset);

	gdt::init ();

	pic::remap (idt::PICM_OFFSET, idt::PICS_OFFSET);
	idt::init ();

	Handler::First(page_fault).register (idt::EXC_PAGE_FAULT)?;
	Handler::Normal(double_fault).register (idt::EXC_DOUBLE_FAULT)?;
	Handler::Normal(gp_exception).register (idt::EXC_GENERAL_PROTECTION_FAULT)?;

	time::pit::init ()?;

	kdata::init ();

	mem::init (boot_info);

	Ok(())
}

#[no_mangle]
pub extern "C" fn _start (boot_info: &'static BootInfo) -> !
{
	// so you can tell when compiler output stops
	eprintln! ("=========================== start kernel debug output ===========================");
	eprintln! ("{:#x?}", boot_info);
	eprintln! ("{:?}", boot_info as *const _);
	eprintln! ("{:?}", init as *const u8);

	init (boot_info).expect ("kernel init failed");

	println! ("epoch v0.0.1");

	sti ();

	//test ();

	loop {
		hlt ();
	}
}

const order_size: usize = 0x100;

fn test ()
{
	eprintln! ("=============================== start test output ===============================");
	/*let mem = [0; order_size * 512];
	let addr = mem.as_slice ().as_ptr () as u64;
	let mut allocator = unsafe { phys_alloc::BuddyAllocator::new (VirtAddr::new (addr), VirtAddr::new (addr + (order_size as u64) * 512), order_size) };
	eprintln! ("Start addr: {:x}\nEnd eddr: {:x}\nSize: {:x}", addr, addr + (order_size as u64) * 512, order_size);
	unsafe
	{
		let a1 = allocator.alloc (1).unwrap ();
		let a2 = allocator.alloc (1).unwrap ();
		let a3 = allocator.alloc (1).unwrap ();
		let a4 = allocator.alloc (1).unwrap ();
		let a5 = allocator.alloc (1).unwrap ();
		eprintln! ("{:#?}", a1);
		eprintln! ("{:#?}", a2);
		eprintln! ("{:#?}", a3);
		eprintln! ("{:#?}", a4);
		eprintln! ("{:#?}", a5);
		allocator.dealloc (a1);
		let a6 = allocator.alloc (1).unwrap ();
		let a7 = allocator.alloc (1).unwrap ();
		let a8 = allocator.alloc (1).unwrap ();
		let a9 = allocator.alloc (1).unwrap ();
		eprintln! ("{:#?}", a6);
		eprintln! ("{:#?}", a7);
		eprintln! ("{:#?}", a8);
		eprintln! ("{:#?}", a9);
		let a9 = allocator.orealloc (a9, 2).unwrap ();
		eprintln! ("{:#?}", a9);
		let a10 = allocator.alloc (1).unwrap ();
		eprintln! ("{:#?}", a10);
		let a9 = allocator.orealloc (a9, 1).unwrap ();
		eprintln! ("{:#?}", a9);
		let a11 = allocator.oalloc (1).unwrap ();
		eprintln! ("{:#?}", a11);
		allocator.dealloc (a2);
		allocator.dealloc (a3);
		allocator.dealloc (a4);
		allocator.dealloc (a5);
		allocator.dealloc (a6);
		allocator.dealloc (a7);
		allocator.dealloc (a8);
		allocator.dealloc (a9);
		allocator.dealloc (a10);
		allocator.dealloc (a11);
	}*/
	let a = Box::new (123);
	let b = Box::new (123);
	let mut c = vec![1, 2, 3];
	c.push (4);
	let mut d: Vec<u8> = Vec::new ();
	for a in 0..(PAGE_SIZE * 4)
	{
		d.push (a as u8);
	}
	eprintln! ("{:?}", d);
	println! ("{:?}", c);
	println! ("{}", *a);
	println! ("{}", *b);
	eprintln! ("test finished");
}
