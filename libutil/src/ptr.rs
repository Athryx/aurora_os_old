use core::ops::{Deref, DerefMut};
use core::borrow::{Borrow, BorrowMut};
use core::convert::{AsMut, AsRef};
use core::marker::PhantomData;

use crate::uses::*;

pub trait UniquePtr<T: ?Sized>: Deref<Target = T>
{
	fn ptr(&self) -> *const T;
}

pub trait UniqueMutPtr<T: ?Sized>: UniquePtr<T> + DerefMut<Target = T>
{
	fn ptr_mut(&self) -> *mut T;
}

#[derive(Debug)]
pub struct UniqueRef<'a, T: ?Sized>
{
	data: *const T,
	marker: PhantomData<&'a T>,
}

impl<T: ?Sized> UniqueRef<'_, T>
{
	pub fn new(other: &T) -> UniqueRef<T>
	{
		UniqueRef {
			data: other,
			marker: PhantomData,
		}
	}

	pub unsafe fn from_ptr<'a>(ptr: *const T) -> UniqueRef<'a, T>
	{
		UniqueRef {
			data: ptr,
			marker: PhantomData,
		}
	}

	pub unsafe fn unbound<'a>(self) -> UniqueRef<'a, T>
	{
		UniqueRef::from_ptr(self.ptr())
	}
}

impl<T: ?Sized> Deref for UniqueRef<'_, T>
{
	type Target = T;

	fn deref(&self) -> &Self::Target
	{
		unsafe { self.data.as_ref().unwrap() }
	}
}

impl<T: ?Sized> Borrow<T> for UniqueRef<'_, T>
{
	fn borrow(&self) -> &T
	{
		self
	}
}

impl<T: ?Sized> AsRef<T> for UniqueRef<'_, T>
{
	fn as_ref(&self) -> &T
	{
		self
	}
}

impl<T: ?Sized> UniquePtr<T> for UniqueRef<'_, T>
{
	fn ptr(&self) -> *const T
	{
		self.data
	}
}

// cloning is unsafe
impl<T: ?Sized> Clone for UniqueRef<'_, T>
{
	fn clone(&self) -> Self
	{
		UniqueRef {
			data: self.data,
			marker: PhantomData,
		}
	}
}

#[derive(Debug)]
pub struct UniqueMut<'a, T: ?Sized>
{
	data: *mut T,
	marker: PhantomData<&'a mut T>,
}

impl<T: ?Sized> UniqueMut<'_, T>
{
	pub fn new(other: &mut T) -> UniqueMut<T>
	{
		UniqueMut {
			data: other,
			marker: PhantomData,
		}
	}

	pub unsafe fn from_ptr<'a>(ptr: *mut T) -> UniqueMut<'a, T>
	{
		UniqueMut {
			data: ptr,
			marker: PhantomData,
		}
	}

	pub fn downgrade<'a>(self) -> UniqueRef<'a, T>
	where
		Self: 'a,
	{
		unsafe { UniqueRef::from_ptr(self.data) }
	}

	pub unsafe fn unbound<'a>(self) -> UniqueMut<'a, T>
	{
		UniqueMut::from_ptr(self.ptr_mut())
	}
}

impl<T: ?Sized> Deref for UniqueMut<'_, T>
{
	type Target = T;

	fn deref(&self) -> &Self::Target
	{
		unsafe { self.data.as_ref().unwrap() }
	}
}

impl<T: ?Sized> DerefMut for UniqueMut<'_, T>
{
	fn deref_mut(&mut self) -> &mut Self::Target
	{
		unsafe { self.data.as_mut().unwrap() }
	}
}

impl<T: ?Sized> Borrow<T> for UniqueMut<'_, T>
{
	fn borrow(&self) -> &T
	{
		self
	}
}

impl<T: ?Sized> BorrowMut<T> for UniqueMut<'_, T>
{
	fn borrow_mut(&mut self) -> &mut T
	{
		self
	}
}

impl<T: ?Sized> AsRef<T> for UniqueMut<'_, T>
{
	fn as_ref(&self) -> &T
	{
		self
	}
}

impl<T: ?Sized> AsMut<T> for UniqueMut<'_, T>
{
	fn as_mut(&mut self) -> &mut T
	{
		self
	}
}

impl<T: ?Sized> UniquePtr<T> for UniqueMut<'_, T>
{
	fn ptr(&self) -> *const T
	{
		self.data
	}
}

impl<T: ?Sized> UniqueMutPtr<T> for UniqueMut<'_, T>
{
	fn ptr_mut(&self) -> *mut T
	{
		self.data as *const T as *mut T
	}
}

// cloning is unsafe
impl<T> Clone for UniqueMut<'_, T>
{
	fn clone(&self) -> Self
	{
		UniqueMut {
			data: self.data,
			marker: PhantomData,
		}
	}
}
