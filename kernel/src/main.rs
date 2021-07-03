#![no_std]
#![no_main]

#![feature(asm)]
#![feature(const_fn_trait_bound)]
#![feature(maybe_uninit_uninit_array)]
#![feature(array_methods)]
#![feature(alloc_error_handler)]
#![feature(try_trait)]
#![feature(arc_new_cyclic)]
#![feature(const_btree_new)]
#![feature(alloc_prelude)]
#![feature(allocator_api)]
#![feature(map_first_last)]
#![feature(stmt_expr_attributes)]
#![feature(map_try_insert)]

#![allow(non_upper_case_globals)]
#![allow(dead_code)]
#![allow(clippy::suspicious_else_formatting)]

extern crate alloc;

mod arch;
mod int;
mod util;
mod sched;
mod mem;
mod syscall;
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
use util::AvlTree;
use mem::*;
use mem::phys_alloc::zm;
use alloc::boxed::Box;
use alloc::collections::*;
use util::AtomicU128;
use alloc::vec;
use upriv::{PrivLevel, IOPRIV_UID};

#[panic_handler]
fn panic(info: &PanicInfo) -> !
{
	rprintln! ("{}", info);
	println! ("{}", info);

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
	panic! (r"page fault accessing virtual address {:x}
page fault during {} {}
non present page: {}
reserved bit set: {}
registers:
{:x?}",
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

	syscall::init ();

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

	sti_safe ();

	Process::from_elf (*consts::INITFS, PrivLevel::new (IOPRIV_UID), "initfs".to_string ()).unwrap ();

	test ();

	loop {
		hlt ();
	}
}

use core::cell::Cell;
use core::fmt::{self, Formatter, Display};
use util::*;

#[derive(Debug)]
struct TreeTest
{
	key: usize,
	val: usize,
	left: Cell<*const Self>,
	right: Cell<*const Self>,
	parent: Cell<*const Self>,
	bf: Cell<i8>,
}

impl TreeTest
{
	fn new () -> MemCell<Self>
	{
		let out = Box::new (TreeTest {
			key: 0,
			val: 0,
			left: Cell::new (null ()),
			right: Cell::new (null ()),
			parent: Cell::new (null ()),
			bf: Cell::new (0),
		});
		MemCell::new (Box::leak (out) as *mut _)
	}
}

impl Display for TreeTest
{
	fn fmt (&self, f: &mut Formatter<'_>) -> fmt::Result
	{
		write! (f, "{}:{}", self.key, self.bf.get ()).unwrap ();
		Ok(())
	}
}

impl_tree_node! (usize, TreeTest, parent, left, right, key, bf);

// just for test
static mut join_tid: usize = 0;

fn test ()
{
	let stopf = false;
	if stopf { cli (); }

	let mut num = 141;
	let _test_closure = move || {
		eprintln! ("test closure ran");
		eprintln! ("num {}", num);
		num += 1;
		eprintln! ("num + 1 {}", num);
		thread_c ().block (ThreadState::Destroy);
	};

	let atom = AtomicU128::new (0);
	for a in 0..20420
	{
		atom.store (a);
		assert_eq! (atom.load (), a);
	}

	let mut tree = AvlTree::new ();
	tree.insert (0, TreeTest::new ()).unwrap ();
	eprintln! ("{}", tree);
	tree.insert (5, TreeTest::new ()).unwrap ();
	eprintln! ("{}", tree);
	tree.insert (10, TreeTest::new ()).unwrap ();
	eprintln! ("{}", tree);
	tree.insert (999, TreeTest::new ()).unwrap ();
	eprintln! ("{}", tree);
	tree.insert (555, TreeTest::new ()).unwrap ();
	eprintln! ("{}", tree);

	eprintln! ("{:?}", *tree.get (&0).unwrap ());
	eprintln! ("{:?}", *tree.get (&5).unwrap ());
	eprintln! ("{:?}", *tree.get (&10).unwrap ());
	eprintln! ("{:?}", *tree.get (&555).unwrap ());
	eprintln! ("{:?}", *tree.get (&999).unwrap ());

	tree.remove (&5).unwrap ();
	eprintln! ("{}", tree);
	tree.remove (&555).unwrap ();
	eprintln! ("{}", tree);
	tree.remove (&0).unwrap ();
	eprintln! ("{}", tree);
	tree.remove (&10).unwrap ();
	eprintln! ("{}", tree);
	tree.remove (&999).unwrap ();
	eprintln! ("{}", tree);

	let vec = NLVec::new ();
	vec.push (3);
	vec.push (2);
	vec.push (5);
	vec.remove (1);
	eprintln! ("{:?}", vec);

	if stopf
	{
		loop
		{
			cli ();
			hlt ();
		}
	}

	let temp = Futex::new (0);
	{
		let mut a = temp.lock ();
		*a += 2;
	}

	eprintln! ("{}", *temp.lock ());

	unsafe
	{
		join_tid = proc_c ().new_thread (test_thread_1, Some("alloc_test_thread".to_string ())).unwrap ();
	}
	proc_c ().new_thread (test_thread_2, Some("join_test_thread".to_string ())).unwrap ();
	for _ in 0..10
	{
		proc_c ().new_thread (test_alloc_thread, Some("alloc_test_thread".to_string ())).unwrap ();
	}
	/*unsafe
	{
		proc_c ().new_thread (core::mem::transmute (&test_closure), Some("closure_test_thread".to_string ())).unwrap ();
	}*/
}

fn test_thread_2 ()
{
	eprintln! ("join test thread started");
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
	thread_c ().block (ThreadState::Destroy);
}

fn test_alloc_thread ()
{
	loop
	{
		let _a = Box::new (0);
		let _b = Box::new (0);
		let _c = Box::new (0);
		let _d = Box::new (0);
		let _e = Box::new (0);
		let _f = Box::new (0);
		let _g = Box::new (0);
		let _h = Box::new (0);
		let _i = Box::new (0);
		let _j = Box::new (0);
		let _k = Box::new (0);
		let _l = Box::new (0);
		let _m = Box::new (0);
		let _n = Box::new (0);
		let _o = Box::new (0);
		let _p = Box::new (0);
		let _q = Box::new (0);
		let _s = Box::new (0);
		let _t = Box::new (0);
		let _u = Box::new (0);
		let _v = Box::new (0);
		let _w = Box::new (0);
		let _x = Box::new (0);
		let _y = Box::new (0);
		let _z = Box::new (0);
		println! ("alloc test done");
		thread_c ().block (ThreadState::Destroy);
	}
}
