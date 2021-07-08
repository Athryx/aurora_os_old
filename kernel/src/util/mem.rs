use crate::uses::*;
use core::ops::Deref;

#[derive(Debug)]
pub struct MemOwner<T> (*const T);

impl<T> MemOwner<T>
{
	pub unsafe fn new (ptr: *const T) -> Self
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
