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

// TODO: probably eliminate
mod error;
pub use error::{Error, Err};

mod imutex;
pub use imutex::{IMutex, IMutexGuard};

mod futex;
pub use futex::{Futex, FutexGuard, RWFutex, RWFutexReadGuard, RWFutexWriteGuard};

// TODO: use macros to make shorter
pub mod ptr;
pub use ptr::{UniqueRef, UniqueMut, UniquePtr, UniqueMutPtr};

// TODO: figure out how to move to user space
mod mem;
pub use mem::MemOwner;

pub fn optac<T, F> (opt: Option<T>, f: F) -> bool
	where F: FnOnce(T) -> bool
{
	match opt
	{
		Some(val) => f (val),
		None => false,
	}
}

pub fn optnac<T, F> (opt: Option<T>, f: F) -> bool
	where F: FnOnce(T) -> bool
{
	match opt
	{
		Some(val) => f (val),
		None => true,
	}
}

pub fn aligned_nonnull<T> (ptr: *const T) -> bool
{
	core::mem::align_of::<T> () == align_of (ptr as usize) && !ptr.is_null ()
}

fn to_heap<V> (object: V) -> *mut V
{
	Box::into_raw (Box::new (object))
}

unsafe fn from_heap<V> (ptr: *const V) -> V
{
	*Box::from_raw (ptr as *mut _)
}

// TODO: make this not require defualt
pub fn copy_to_heap<T: Copy + Default> (slice: &[T]) -> Vec<T>
{
	let mut out = Vec::with_capacity (slice.len ());
	out.resize (slice.len (), T::default ());
	out.copy_from_slice (slice);
	out
}

use alloc::alloc::Layout;

pub const fn mlayout_of<T> () -> Layout
{
	unsafe { Layout::from_size_align_unchecked (size_of::<T> (), core::mem::align_of::<T> ()) }
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
