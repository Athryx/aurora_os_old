use crate::uses::*;
use core::str::Utf8Error;

pub mod io;

pub mod misc;

pub use libutil::collections::{AvlTree, LinkedList, ListNode, NLVec, NLVecMap, TreeNode};

// TODO: probably eliminate
mod error;
pub use error::{Err, Error};

mod imutex;
pub use imutex::{IMutex, IMutexGuard};
pub use libutil::futex::{Futex, FutexGuard, RWFutex, RWFutexReadGuard, RWFutexWriteGuard};
// TODO: use macros to make shorter
pub use libutil::ptr::{UniqueMut, UniqueMutPtr, UniquePtr, UniqueRef};
pub use libutil::mem::MemOwner;
use libutil::mem::Allocation;
use libutil::UtilCalls;

use crate::sched::{proc_c, thread_c, tlist, FutexId, ThreadState};
use crate::mem::phys_alloc::zm;

pub static CALLS: Calls = Calls();

pub struct Calls();

impl UtilCalls for Calls
{
	// NOTE: chage if kernel ever blocks on shared memory
	fn block(&self, id: usize)
	{
		proc_c().futex().block(id);
	}

	fn unblock(&self, id: usize)
	{
		proc_c().futex().unblock(id, 1);
	}

	fn alloc(&self, size: usize) -> Option<Allocation>
	{
		zm.alloc(size)
	}

	fn dealloc(&self, mem: Allocation)
	{
		unsafe {
			zm.dealloc(mem);
		}
	}
}

pub unsafe fn from_cstr<'a>(ptr: *const u8) -> Result<&'a str, Utf8Error>
{
	let mut len = 0;
	let start = ptr;

	loop {
		if *ptr.add(len) != 0 {
			len += 1;
		} else {
			break;
		}
	}

	let slice = core::slice::from_raw_parts(start, len);
	core::str::from_utf8(slice)
}

// code from some reddit post
#[macro_export]
macro_rules! init_array (
	($ty:ty, $len:expr, $val:expr) => (
		{
			use core::mem::MaybeUninit;
			let mut array: [MaybeUninit<$ty>; $len] = MaybeUninit::uninit_array ();
			for a in array.iter_mut() {
				#[allow(unused_unsafe)]
				unsafe { ::core::ptr::write(a.as_mut_ptr (), $val); }
			}
			#[allow(unused_unsafe)]
			unsafe { core::mem::transmute::<[MaybeUninit<$ty>; $len], [$ty; $len]> (array) }
		}
	)
);
