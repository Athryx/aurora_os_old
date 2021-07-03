use crate::uses::*;

pub mod io;

pub mod misc;

mod linked_list;
pub use linked_list::{LinkedList, ListNode, Node};

mod tree;
pub use tree::{AvlTree, TreeNode};

mod nlvec;
pub use nlvec::NLVec;

mod nlvecmap;
pub use nlvecmap::NLVecMap;

mod error;
pub use error::{Error, Err};

mod imutex;
pub use imutex::{IMutex, IMutexGuard};

mod futex;
pub use futex::{Futex, FutexGaurd, RWFutex, RWFutexReadGuard, RWFutexWriteGuard};

pub mod cell;
pub use cell::{MemCell, UniqueRef, UniqueMut, UniquePtr, UniqueMutPtr};

mod atomic;
pub use atomic::AtomicU128;
	
fn to_heap<V> (object: V) -> *mut V
{
	Box::into_raw (Box::new (object))
}

unsafe fn from_heap<V> (ptr: *const V) -> V
{
	*Box::from_raw (ptr as *mut _)
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
