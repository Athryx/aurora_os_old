use core::sync::atomic::{AtomicIsize, Ordering};
use core::ops::{Deref, DerefMut};

use crate::uses::*;

// TODO: find out if this structure is even necessary
#[derive(Debug, Clone, Copy)]
pub struct BorrowError;

#[derive(Debug)]
pub struct MemCell<T: ?Sized>
{
	data: *mut T,
	// positive is reader count, negative is writer count
	rw: AtomicIsize,
}

impl<T: ?Sized> MemCell<T>
{
	pub fn new(val: *mut T) -> Self
	{
		MemCell {
			data: val,
			rw: AtomicIsize::new(0),
		}
	}

	pub fn try_borrow(&self) -> Result<Reader<T>, BorrowError>
	{
		Reader::new(self)
	}

	pub fn borrow(&self) -> Reader<T>
	{
		self.try_borrow()
			.expect("could not borrow MemCell as immutable")
	}

	pub fn try_borrow_mut(&self) -> Result<Writer<T>, BorrowError>
	{
		Writer::new(self)
	}

	pub fn borrow_mut(&self) -> Writer<T>
	{
		self.try_borrow_mut()
			.expect("could not borrow MemCell as mutable")
	}

	pub fn ptr(&self) -> *const T
	{
		self.data
	}

	pub fn ptr_mut(&self) -> *mut T
	{
		self.data
	}
}

unsafe impl<T> Send for MemCell<T> {}
unsafe impl<T> Sync for MemCell<T> {}

#[derive(Debug)]
pub struct Reader<'a, T: ?Sized>
{
	data: *const T,
	cell: &'a MemCell<T>,
}

impl<T: ?Sized> Reader<'_, T>
{
	fn new(cell: &MemCell<T>) -> Result<Reader<T>, BorrowError>
	{
		let closure = |num| {
			if num < 0 {
				None
			} else {
				Some(num + 1)
			}
		};

		// TODO: figre out if seqcst is needed
		cell.rw
			.fetch_update(Ordering::SeqCst, Ordering::SeqCst, closure)
			.map_err(|_| BorrowError)?;
		Ok(Reader {
			data: cell.data,
			cell,
		})
	}
}

impl<T: ?Sized> Deref for Reader<'_, T>
{
	type Target = T;

	fn deref(&self) -> &Self::Target
	{
		unsafe { self.data.as_ref().unwrap() }
	}
}

impl<T: ?Sized> Drop for Reader<'_, T>
{
	fn drop(&mut self)
	{
		self.cell.rw.fetch_sub(1, Ordering::SeqCst);
	}
}

#[derive(Debug)]
pub struct Writer<'a, T: ?Sized>
{
	data: *mut T,
	cell: &'a MemCell<T>,
}

impl<T: ?Sized> Writer<'_, T>
{
	fn new(cell: &MemCell<T>) -> Result<Writer<T>, BorrowError>
	{
		// TODO: figre out if this is correct ordering
		cell.rw
			.compare_exchange(0, -1, Ordering::SeqCst, Ordering::SeqCst)
			.map_err(|_| BorrowError)?;
		Ok(Writer {
			data: cell.data,
			cell,
		})
	}
}

impl<T: ?Sized> Deref for Writer<'_, T>
{
	type Target = T;

	fn deref(&self) -> &Self::Target
	{
		unsafe { self.data.as_ref().unwrap() }
	}
}

impl<T: ?Sized> DerefMut for Writer<'_, T>
{
	fn deref_mut(&mut self) -> &mut Self::Target
	{
		unsafe { self.data.as_mut().unwrap() }
	}
}

impl<T: ?Sized> Drop for Writer<'_, T>
{
	fn drop(&mut self)
	{
		self.cell.rw.store(0, Ordering::SeqCst);
	}
}
