use crate::uses::*;

pub mod io;

pub mod misc;

pub use libutil::collections::{LinkedList, ListNode};

pub use libutil::collections::{AvlTree, TreeNode};

pub use libutil::collections::NLVec;

pub use libutil::collections::NLVecMap;

// TODO: probably eliminate
mod error;
pub use error::{Error, Err};

mod imutex;
pub use imutex::{IMutex, IMutexGuard};

pub use libutil::futex::{Futex, FutexGuard, RWFutex, RWFutexReadGuard, RWFutexWriteGuard};

// TODO: use macros to make shorter
pub use libutil::ptr::{UniqueRef, UniqueMut, UniquePtr, UniqueMutPtr};

pub use libutil::memown::MemOwner;

use libutil::{UtilCalls, mem::Allocation};
use crate::sched::{thread_c, proc_c, ThreadState};
use crate::mem::phys_alloc::zm;

pub static CALLS: Calls = Calls();

pub struct Calls();

impl UtilCalls for Calls
{
	fn block (&self, addr: usize)
	{
		thread_c ().block (ThreadState::FutexBlock(addr));
	}
	
	fn unblock (&self, addr: usize)
	{
		proc_c ().futex_move (addr, ThreadState::Ready, 1);
	}

	fn alloc (&self, size: usize) -> Option<Allocation>
	{
		zm.alloc (size)
	}

	fn dealloc (&self, mem: Allocation)
	{
		unsafe
		{
			zm.dealloc (mem);
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
