#![no_std]
#![no_main]

#![feature(asm)]
#![feature(const_fn)]
#![feature(maybe_uninit_uninit_array)]
#![feature(array_methods)]
#![feature(alloc_error_handler)]
#![feature(try_trait)]
#![feature(arc_new_cyclic)]
#![feature(const_btree_new)]
#![feature(alloc_prelude)]
#![feature(allocator_api)]
#![feature(map_first_last)]

#![allow(non_upper_case_globals)]
#![allow(dead_code)]
#![allow(clippy::suspicious_else_formatting)]

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
mod mb2;
mod consts;
mod upriv;

use uses::*;
use core::panic::PanicInfo;
use mb2::BootInfo;
use arch::x64::*;
use sched::*;
use int::*;
use int::idt::Handler;
use util::misc;
use mem::*;
use mem::phys_alloc::zm;
use alloc::boxed::Box;
use alloc::collections::*;
use alloc::vec;
use upriv::{PrivLevel, IOPRIV_UID};

#[panic_handler]
fn panic(info: &PanicInfo) -> !
{
	println! ("{}", info);
	eprintln! ("{}", info);

	loop {
		cli ();
		hlt ();
	}
}

fn double_fault (_: &Registers, _: u64) -> Option<&Registers>
{
	println! ("double fault");
	None
}

fn gp_exception (_: &Registers, _: u64) -> Option<&Registers>
{
	println! ("general protection exception");
	None
}

fn page_fault (regs: &Registers, code: u64) -> Option<&Registers>
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
	else if code & idt::PAGE_FAULT_WRITE != 0
	{
		"write"
	}
	else
	{
		"read"
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

fn init (boot_info: &BootInfo) -> Result<(), util::Err>
{
	util::io::WRITER.lock ().clear ();
	misc::init (*consts::KERNEL_VMA as u64);

	gdt::init ();

	pic::remap (idt::PICM_OFFSET, idt::PICS_OFFSET);
	idt::init ();

	Handler::First(page_fault).register (idt::EXC_PAGE_FAULT)?;
	Handler::Normal(double_fault).register (idt::EXC_DOUBLE_FAULT)?;
	Handler::Normal(gp_exception).register (idt::EXC_GENERAL_PROTECTION_FAULT)?;

	time::pit::init ()?;

	kdata::init ();

	mem::init (boot_info);

	sched::init ()?;

	Ok(())
}

#[no_mangle]
pub extern "C" fn _start (boot_info_addr: usize) -> !
{
	bochs_break ();
	// so you can tell when compiler output stops
	eprintln! ("=========================== start kernel debug output ===========================");
	let boot_info = unsafe { BootInfo::new (boot_info_addr) };

	init (&boot_info).expect ("kernel init failed");

	println! ("epoch v0.0.1");

	//gdt::tss.lock ().rsp0 = align_up (get_rsp () + 0x100, 16) as u64;

	sti_safe ();

	Process::from_elf (*consts::INITFS, PrivLevel::new (IOPRIV_UID), "initfs".to_string ()).unwrap ();

	//test ();

	loop {
		hlt ();
	}
}

// just for test
static mut join_tid: usize = 0;

fn test ()
{
	unsafe
	{
		join_tid = proc_c ().new_thread (test_thread_1, Some("alloc_test_thread".to_string ())).unwrap ();
	}
	proc_c ().new_thread (test_thread_2, Some("join_test_thread".to_string ())).unwrap ();
}

fn test_thread_2 ()
{
	eprintln! ("join test thread started");
	loop
	{
		hlt ();
	}
	thread_c ().block (ThreadState::Join(unsafe { join_tid }));
	eprintln! ("finished joining");
	thread_c ().block (ThreadState::Destroy);
}

const order_size: usize = 0x100;

fn test_thread_1 ()
{
	eprintln! ("=============================== start test output ===============================");
	unsafe
	{
		let a1 = zm.alloc (1).unwrap ();
		let a2 = zm.alloc (1).unwrap ();
		let a3 = zm.alloc (1).unwrap ();
		let a4 = zm.alloc (1).unwrap ();
		let a5 = zm.alloc (1).unwrap ();
		eprintln! ("{:#?}", a1);
		eprintln! ("{:#?}", a2);
		eprintln! ("{:#?}", a3);
		eprintln! ("{:#?}", a4);
		eprintln! ("{:#?}", a5);
		zm.dealloc (a1);
		let a6 = zm.alloc (1).unwrap ();
		let a7 = zm.alloc (1).unwrap ();
		let a8 = zm.alloc (1).unwrap ();
		let a9 = zm.alloc (1).unwrap ();
		eprintln! ("{:#?}", a6);
		eprintln! ("{:#?}", a7);
		eprintln! ("{:#?}", a8);
		eprintln! ("{:#?}", a9);
		let a9 = zm.orealloc (a9, 2).unwrap ();
		eprintln! ("{:#?}", a9);
		let a10 = zm.alloc (1).unwrap ();
		eprintln! ("{:#?}", a10);
		let a9 = zm.orealloc (a9, 1).unwrap ();
		eprintln! ("{:#?}", a9);
		let a11 = zm.oalloc (1).unwrap ();
		eprintln! ("{:#?}", a11);
		zm.dealloc (a2);
		zm.dealloc (a3);
		zm.dealloc (a4);
		zm.dealloc (a5);
		zm.dealloc (a6);
		zm.dealloc (a7);
		zm.dealloc (a8);
		zm.dealloc (a9);
		zm.dealloc (a10);
		zm.dealloc (a11);
	}
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
	loop
	{
		hlt ();
	}
	thread_c ().block (ThreadState::Destroy);
}
