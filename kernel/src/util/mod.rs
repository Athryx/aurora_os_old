use crate::uses::*;

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
