use crate::uses::*;
use core::sync::atomic::{AtomicIsize, Ordering};
use core::ops::{Deref, DerefMut};
use core::marker::PhantomData;

#[derive(Debug, Clone, Copy)]
pub struct BorrowError;

#[derive(Debug)]
pub struct MemCell<T>
{
	data: *mut T,
	// positive is reader count, negative is writer count
	rw: AtomicIsize,
}

impl<T> MemCell<T>
{
	pub fn new (val: *mut T) -> Self
	{
		MemCell {
			data: val,
			rw: AtomicIsize::new (0),
		}
	}

	pub fn try_borrow (&self) -> Result<Reader<T>, BorrowError>
	{
		Reader::new (self)
	}

	pub fn borrow (&self) -> Reader<T>
	{
		self.try_borrow ().expect ("could not borrow MemCell as immutable")
	}

	pub fn try_borrow_mut (&self) -> Result<Writer<T>, BorrowError>
	{
		Writer::new (self)
	}

	pub fn borrow_mut (&self) -> Writer<T>
	{
		self.try_borrow_mut ().expect ("could not borrow MemCell as mutable")
	}

	pub fn ptr (&self) -> *const T
	{
		self.data
	}

	pub fn ptr_mut (&self) -> *mut T
	{
		self.data
	}
}

unsafe impl<T> Send for MemCell<T> {}
unsafe impl<T> Sync for MemCell<T> {}

#[derive(Debug)]
pub struct Reader<'a, T>
{
	data: *const T,
	cell: &'a MemCell<T>,
}

impl<T> Reader<'_, T>
{
	fn new (cell: &MemCell<T>) -> Result<Reader<T>, BorrowError>
	{
		let closure = |num| {
			if num < 0
			{
				None
			}
			else
			{
				Some(num + 1)
			}
		};

		// TODO: figre out if seqcst is needed
		cell.rw.fetch_update (Ordering::SeqCst, Ordering::SeqCst, closure).or_else (|_| Err(BorrowError))?;
		Ok(Reader {
			data: cell.data,
			cell,
		})
	}
}

impl<T> Deref for Reader<'_, T>
{
	type Target = T;

	fn deref (&self) -> &Self::Target
	{
		unsafe
		{
			self.data.as_ref ().unwrap ()
		}
	}
}

impl<T> Drop for Reader<'_, T>
{
	fn drop (&mut self)
	{
		self.cell.rw.fetch_sub (1, Ordering::SeqCst);
	}
}

#[derive(Debug)]
pub struct Writer<'a, T>
{
	data: *mut T,
	cell: &'a MemCell<T>,
}

impl<T> Writer<'_, T>
{
	fn new (cell: &MemCell<T>) -> Result<Writer<T>, BorrowError>
	{
		// TODO: figre out if this is correct ordering
		cell.rw.compare_exchange (0, -1, Ordering::SeqCst, Ordering::SeqCst).or_else (|_| Err(BorrowError))?;
		Ok(Writer {
			data: cell.data,
			cell,
		})
	}
}

impl<T> Deref for Writer<'_, T>
{
	type Target = T;

	fn deref (&self) -> &Self::Target
	{
		unsafe
		{
			self.data.as_ref ().unwrap ()
		}
	}
}

impl<T> DerefMut for Writer<'_, T>
{
	fn deref_mut (&mut self) -> &mut Self::Target
	{
		unsafe
		{
			self.data.as_mut ().unwrap ()
		}
	}
}

impl<T> Drop for Writer<'_, T>
{
	fn drop (&mut self)
	{
		self.cell.rw.store (0, Ordering::SeqCst);
	}
}

pub trait UniquePtr<T>: Deref<Target = T>
{
	fn ptr (&self) -> *const T;
}

pub trait UniqueMutPtr<T>: UniquePtr<T> + DerefMut<Target = T>
{
	fn ptr_mut (&self) -> *mut T;
}

#[derive(Debug)]
pub struct UniqueRef<'a, T>
{
	data: *const T,
	marker: PhantomData<&'a T>,
}

impl<T> UniqueRef<'_, T>
{
	pub fn new (other: &T) -> UniqueRef<T>
	{
		UniqueRef {
			data: other,
			marker: PhantomData,
		}
	}

	pub unsafe fn from_ptr<'a> (ptr: *const T) -> UniqueRef<'a, T>
	{
		UniqueRef {
			data: ptr,
			marker: PhantomData,
		}
	}

	pub unsafe fn unbound<'a> (self) -> UniqueRef<'a, T>
	{
		UniqueRef::from_ptr (self.ptr ())
	}
}

impl<T> Deref for UniqueRef<'_, T>
{
	type Target = T;

	fn deref (&self) -> &Self::Target
	{
		unsafe
		{
			self.data.as_ref ().unwrap ()
		}
	}
}

impl<T> UniquePtr<T> for UniqueRef<'_, T>
{
	fn ptr (&self) -> *const T
	{
		self.data
	}
}

// cloning is unsafe
impl<T> Clone for UniqueRef<'_, T>
{
	fn clone (&self) -> Self
	{
		UniqueRef {
			data: self.data,
			marker: PhantomData,
		}
	}
}

#[derive(Debug)]
pub struct UniqueMut<'a, T>
{
	data: *mut T,
	marker: PhantomData<&'a mut T>
}

impl<T> UniqueMut<'_, T>
{
	pub fn new (other: &mut T) -> UniqueMut<T>
	{
		UniqueMut {
			data: other,
			marker: PhantomData,
		}
	}

	pub unsafe fn from_ptr<'a> (ptr: *mut T) -> UniqueMut<'a, T>
	{
		UniqueMut {
			data: ptr,
			marker: PhantomData,
		}
	}

	pub unsafe fn unbound<'a> (self) -> UniqueMut<'a, T>
	{
		UniqueMut::from_ptr (self.ptr_mut ())
	}
}

impl<T> Deref for UniqueMut<'_, T>
{
	type Target = T;

	fn deref (&self) -> &Self::Target
	{
		unsafe
		{
			self.data.as_ref ().unwrap ()
		}
	}
}

impl<T> DerefMut for UniqueMut<'_, T>
{
	fn deref_mut (&mut self) -> &mut Self::Target
	{
		unsafe
		{
			self.data.as_mut ().unwrap ()
		}
	}
}

impl<T> UniquePtr<T> for UniqueMut<'_, T>
{
	fn ptr (&self) -> *const T
	{
		self.data
	}
}

impl<T> UniqueMutPtr<T> for UniqueMut<'_, T>
{
	fn ptr_mut (&self) -> *mut T
	{
		self.data as *const T as *mut T
	}
}

// cloning is unsafe
impl<T> Clone for UniqueMut<'_, T>
{
	fn clone (&self) -> Self
	{
		UniqueMut {
			data: self.data,
			marker: PhantomData,
		}
	}
}
