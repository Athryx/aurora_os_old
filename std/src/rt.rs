use crate::uses::*;
use libutil::{UtilCalls, mem::Allocation};
use sys::{futex_block, futex_unblock, realloc, ReallocOptions};

static UTIL_CALLS: Calls = Calls();

struct Calls();

impl UtilCalls for Calls
{
	fn block (&self, addr: usize)
	{
		futex_block (addr);
	}

	fn unblock (&self, addr: usize)
	{
		futex_unblock (addr, 1);
	}

	fn alloc (&self, size: usize) -> Option<Allocation>
	{
		let options = ReallocOptions::READ | ReallocOptions::WRITE;
		let (addr, len) = unsafe { realloc (0, size, 0, options).ok ()? };
		Some(Allocation::new (addr, len))
	}

	fn dealloc (&self, mem: Allocation)
	{
		let options = ReallocOptions::READ | ReallocOptions::WRITE;
		unsafe
		{
			realloc (mem.as_usize (), 0, 0, options).unwrap ();
		}
	}
}

#[lang = "start"]
fn lang_start<T: 'static> (
	main: fn() -> T,
	_argc: isize,
	_argv: *const *const u8,
) -> isize
{
	unsafe
	{
		libutil::init (&UTIL_CALLS);
	}

	main ();
	loop {}
}
