use crate::uses::*;
use core::ops::Deref;
use core::ptr::NonNull;
use alloc::alloc::{Global, Allocator};
use crate::misc::mlayout_of;

#[derive(Debug)]
pub struct MemOwner<T> (*const T);

impl<T> MemOwner<T>
{
	pub fn new (data: T) -> Self
	{
		let layout = mlayout_of::<T> ();

		let mem = Global.allocate (layout).expect ("out of memory for MemOwner");
		let ptr = mem.as_ptr () as *mut T;

		unsafe
		{
			core::ptr::write (ptr, data);
			Self::from_raw (ptr)
		}
	}

	pub unsafe fn from_raw (ptr: *const T) -> Self
	{
		MemOwner(ptr)
	}

	pub unsafe fn clone (&self) -> Self
	{
		MemOwner(self.0)
	}

	pub fn ptr (&self) -> *const T
	{
		self.0
	}

	pub fn ptr_mut (&self) -> *mut T
	{
		self.0 as *mut T
	}

	pub unsafe fn dealloc (self)
	{
		let ptr = NonNull::new (self.ptr_mut ()).unwrap ().cast ();
		Global.deallocate (ptr, mlayout_of::<T> ());
	}
}

impl<T> Deref for MemOwner<T>
{
	type Target = T;

	fn deref (&self) -> &Self::Target
	{
		unsafe
		{
			self.0.as_ref ().unwrap ()
		}
	}
}

unsafe impl<T> Send for MemOwner<T> {}
