use crate::uses::*;
use core::sync::atomic::{AtomicBool, AtomicUsize, AtomicIsize, Ordering};
use core::ops::{Deref, DerefMut};
use core::cell::UnsafeCell;
use crate::{block, unblock};

#[derive(Debug)]
pub struct Futex<T>
{
	acquired: AtomicBool,
	waiting: AtomicUsize,
	data: UnsafeCell<T>,
}

impl<T> Futex<T>
{
	pub const fn new (data: T) -> Self
	{
		Futex {
			acquired: AtomicBool::new (false),
			waiting: AtomicUsize::new (0),
			data: UnsafeCell::new (data),
		}
	}

	pub fn lock (&self) -> FutexGuard<T>
	{
		loop
		{
			match self.try_lock ()
			{
				Ok(guard) => return guard,
				Err(_) => {
					self.waiting.fetch_add (1, Ordering::Relaxed);
					block (self as *const _ as usize);
				},
			}
		}
	}

	pub fn try_lock (&self) -> Result<FutexGuard<T>, ()>
	{
		let acq = self.acquired.swap (true, Ordering::Relaxed);
		if acq
		{
			Err(())
		}
		else
		{
			Ok(FutexGuard::new (self))
		}
	}

	pub fn get_mut (&mut self) -> &mut T
	{
		unsafe
		{
			self.data.get ().as_mut ().unwrap ()
		}
	}

	pub fn into_inner (self) -> T
	{
		self.data.into_inner ()
	}
}

unsafe impl<T> Send for Futex<T> {}
unsafe impl<T> Sync for Futex<T> {}

#[derive(Debug)]
pub struct FutexGuard<'a, T> (&'a Futex<T>);

impl <'a, T> FutexGuard<'a, T>
{
	pub fn new (futex: &'a Futex<T>) -> Self
	{
		FutexGuard(futex) 
	}
}

impl<T> Deref for FutexGuard<'_, T>
{
	type Target = T;

	fn deref (&self) -> &Self::Target
	{
		unsafe
		{
			self.0.data.get ().as_ref ().unwrap ()
		}
	}
}

impl<T> DerefMut for FutexGuard<'_, T>
{
	fn deref_mut (&mut self) -> &mut Self::Target
	{
		unsafe
		{
			self.0.data.get ().as_mut ().unwrap ()
		}
	}
}

impl<T> Drop for FutexGuard<'_, T>
{
	fn drop (&mut self)
	{
		self.0.acquired.store (false, Ordering::Relaxed);

		let closure = |n| {
			if n == 0
			{
				None
			}
			else
			{
				Some(n - 1)
			}
		};

		if self.0.waiting.fetch_update (Ordering::Relaxed, Ordering::Relaxed, closure).is_ok ()
		{
			unblock (self.0 as *const _ as usize);
		}
	}
}

#[derive(Debug)]
pub struct RWFutex<T>
{
	// positive is reader count, negative is writer count
	count: AtomicIsize,
	waiting: AtomicUsize,
	data: UnsafeCell<T>,
}

impl<T> RWFutex<T>
{
	pub const fn new (data: T) -> Self
	{
		RWFutex {
			count: AtomicIsize::new (0),
			waiting: AtomicUsize::new (0),
			data: UnsafeCell::new (data),
		}
	}

	pub fn read (&self) -> RWFutexReadGuard<T>
	{
		loop
		{
			match self.try_read ()
			{
				Ok(guard) => return guard,
				Err(_) => {
					self.waiting.fetch_add (1, Ordering::Relaxed);
					block (self as *const _ as usize);
				},
			}
		}
	}

	pub fn write (&self) -> RWFutexWriteGuard<T>
	{
		loop
		{
			match self.try_write ()
			{
				Ok(guard) => return guard,
				Err(_) => {
					self.waiting.fetch_add (1, Ordering::Relaxed);
					block (self as *const _ as usize);
				},
			}
		}
	}

	pub fn try_read (&self) -> Result<RWFutexReadGuard<T>, ()>
	{
		let acq = self.count.fetch_update (Ordering::Relaxed, Ordering::Relaxed, |n| {
			if n >= 0
			{
				Some(n + 1)
			}
			else
			{
				None
			}
		});
		match acq
		{
			Ok(_) => Ok(RWFutexReadGuard::new (self)),
			Err(_) => Err(()),
		}
	}

	pub fn try_write (&self) -> Result<RWFutexWriteGuard<T>, ()>
	{
		if self.count.compare_exchange (0, -1, Ordering::Relaxed, Ordering::Relaxed).is_ok ()
		{
			Ok(RWFutexWriteGuard::new (self))
		}
		else
		{
			Err(())
		}
	}

	pub fn get_mut (&mut self) -> &mut T
	{
		unsafe
		{
			self.data.get ().as_mut ().unwrap ()
		}
	}

	pub fn into_inner (self) -> T
	{
		self.data.into_inner ()
	}
}

unsafe impl<T> Send for RWFutex<T> {}
unsafe impl<T> Sync for RWFutex<T> {}

#[derive(Debug)]
pub struct RWFutexReadGuard<'a, T> (&'a RWFutex<T>);

impl <'a, T> RWFutexReadGuard<'a, T>
{
	pub fn new (futex: &'a RWFutex<T>) -> Self
	{
		RWFutexReadGuard(futex) 
	}
}

impl<T> Deref for RWFutexReadGuard<'_, T>
{
	type Target = T;

	fn deref (&self) -> &Self::Target
	{
		unsafe
		{
			self.0.data.get ().as_ref ().unwrap ()
		}
	}
}

impl<T> Drop for RWFutexReadGuard<'_, T>
{
	fn drop (&mut self)
	{
		self.0.count.fetch_sub (1, Ordering::Relaxed);

		let closure = |n| {
			if n == 0
			{
				None
			}
			else
			{
				Some(n - 1)
			}
		};

		if self.0.waiting.fetch_update (Ordering::Relaxed, Ordering::Relaxed, closure).is_ok ()
		{
			unblock (self.0 as *const _ as usize);
		}
	}
}

#[derive(Debug)]
pub struct RWFutexWriteGuard<'a, T> (&'a RWFutex<T>);

impl <'a, T> RWFutexWriteGuard<'a, T>
{
	pub fn new (futex: &'a RWFutex<T>) -> Self
	{
		RWFutexWriteGuard(futex) 
	}
}

impl<T> Deref for RWFutexWriteGuard<'_, T>
{
	type Target = T;

	fn deref (&self) -> &Self::Target
	{
		unsafe
		{
			self.0.data.get ().as_ref ().unwrap ()
		}
	}
}

impl<T> DerefMut for RWFutexWriteGuard<'_, T>
{
	fn deref_mut (&mut self) -> &mut Self::Target
	{
		unsafe
		{
			self.0.data.get ().as_mut ().unwrap ()
		}
	}
}

impl<T> Drop for RWFutexWriteGuard<'_, T>
{
	fn drop (&mut self)
	{
		self.0.count.store (0, Ordering::Relaxed);

		let closure = |n| {
			if n == 0
			{
				None
			}
			else
			{
				Some(n - 1)
			}
		};

		if self.0.waiting.fetch_update (Ordering::Relaxed, Ordering::Relaxed, closure).is_ok ()
		{
			unblock (self.0 as *const _ as usize);
		}
	}
}
