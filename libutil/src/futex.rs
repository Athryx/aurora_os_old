use crate::uses::*;
use core::sync::atomic::{AtomicBool, AtomicUsize, AtomicIsize, Ordering};
use core::ops::{Deref, DerefMut};
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use sys::{futex_block, futex_unblock};

#[cfg(not(feature = "kernel"))]
#[derive(Debug)]
pub struct UBlocker();

#[cfg(not(feature = "kernel"))]
impl Blocker for UBlocker
{
	fn block (addr: usize)
	{
		futex_block (addr);
	}

	fn unblock (addr: usize)
	{
		futex_unblock (addr, 1);
	}
}

#[cfg(not(feature = "kernel"))]
pub type Futex<T> = FutexImpl<T, UBlocker>;
#[cfg(not(feature = "kernel"))]
pub type FutexGuard<'a, T> = FutexImplGuard<'a, T, UBlocker>;

#[cfg(not(feature = "kernel"))]
pub type RWFutex<T> = RWFutexImpl<T, UBlocker>;
#[cfg(not(feature = "kernel"))]
pub type RWFutexReadGuard<'a, T> = RWFutexImplReadGuard<'a, T, UBlocker>;
#[cfg(not(feature = "kernel"))]
pub type RWFutexWriteGuard<'a, T> = RWFutexImplWriteGuard<'a, T, UBlocker>;


pub trait Blocker
{
	fn block (addr: usize);
	fn unblock (addr: usize);
}

#[derive(Debug)]
pub struct FutexImpl<T, B: Blocker>
{
	acquired: AtomicBool,
	waiting: AtomicUsize,
	data: UnsafeCell<T>,
	phantom: PhantomData<B>,
}

impl<T, B: Blocker> FutexImpl<T, B>
{
	pub const fn new (data: T) -> Self
	{
		FutexImpl {
			acquired: AtomicBool::new (false),
			waiting: AtomicUsize::new (0),
			data: UnsafeCell::new (data),
			phantom: PhantomData,
		}
	}

	pub fn lock (&self) -> FutexImplGuard<T, B>
	{
		loop
		{
			match self.try_lock ()
			{
				Ok(guard) => return guard,
				Err(_) => {
					self.waiting.fetch_add (1, Ordering::Relaxed);
					B::block (self as *const _ as usize);
				},
			}
		}
	}

	pub fn try_lock (&self) -> Result<FutexImplGuard<T, B>, ()>
	{
		let acq = self.acquired.swap (true, Ordering::Relaxed);
		if acq
		{
			Err(())
		}
		else
		{
			Ok(FutexImplGuard::new (self))
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

unsafe impl<T, B: Blocker> Send for FutexImpl<T, B> {}
unsafe impl<T, B: Blocker> Sync for FutexImpl<T, B> {}

#[derive(Debug)]
pub struct FutexImplGuard<'a, T, B: Blocker> (&'a FutexImpl<T, B>);

impl <'a, T, B: Blocker> FutexImplGuard<'a, T, B>
{
	pub fn new (futex: &'a FutexImpl<T, B>) -> Self
	{
		FutexImplGuard(futex) 
	}
}

impl<T, B: Blocker> Deref for FutexImplGuard<'_, T, B>
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

impl<T, B: Blocker> DerefMut for FutexImplGuard<'_, T, B>
{
	fn deref_mut (&mut self) -> &mut Self::Target
	{
		unsafe
		{
			self.0.data.get ().as_mut ().unwrap ()
		}
	}
}

impl<T, B: Blocker> Drop for FutexImplGuard<'_, T, B>
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
			B::unblock (self.0 as *const _ as usize);
		}
	}
}

#[derive(Debug)]
pub struct RWFutexImpl<T, B: Blocker>
{
	// positive is reader count, negative is writer count
	count: AtomicIsize,
	waiting: AtomicUsize,
	data: UnsafeCell<T>,
	phantom: PhantomData<B>
}

impl<T, B: Blocker> RWFutexImpl<T, B>
{
	pub const fn new (data: T) -> Self
	{
		RWFutexImpl {
			count: AtomicIsize::new (0),
			waiting: AtomicUsize::new (0),
			data: UnsafeCell::new (data),
			phantom: PhantomData,
		}
	}

	pub fn read (&self) -> RWFutexImplReadGuard<T, B>
	{
		loop
		{
			match self.try_read ()
			{
				Ok(guard) => return guard,
				Err(_) => {
					self.waiting.fetch_add (1, Ordering::Relaxed);
					B::block (self as *const _ as usize);
				},
			}
		}
	}

	pub fn write (&self) -> RWFutexImplWriteGuard<T, B>
	{
		loop
		{
			match self.try_write ()
			{
				Ok(guard) => return guard,
				Err(_) => {
					self.waiting.fetch_add (1, Ordering::Relaxed);
					B::block (self as *const _ as usize);
				},
			}
		}
	}

	pub fn try_read (&self) -> Result<RWFutexImplReadGuard<T, B>, ()>
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
			Ok(_) => Ok(RWFutexImplReadGuard::new (self)),
			Err(_) => Err(()),
		}
	}

	pub fn try_write (&self) -> Result<RWFutexImplWriteGuard<T, B>, ()>
	{
		if self.count.compare_exchange (0, -1, Ordering::Relaxed, Ordering::Relaxed).is_ok ()
		{
			Ok(RWFutexImplWriteGuard::new (self))
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

unsafe impl<T, B: Blocker> Send for RWFutexImpl<T, B> {}
unsafe impl<T, B: Blocker> Sync for RWFutexImpl<T, B> {}

#[derive(Debug)]
pub struct RWFutexImplReadGuard<'a, T, B: Blocker> (&'a RWFutexImpl<T, B>);

impl <'a, T, B: Blocker> RWFutexImplReadGuard<'a, T, B>
{
	pub fn new (futex: &'a RWFutexImpl<T, B>) -> Self
	{
		RWFutexImplReadGuard(futex) 
	}
}

impl<T, B: Blocker> Deref for RWFutexImplReadGuard<'_, T, B>
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

impl<T, B: Blocker> Drop for RWFutexImplReadGuard<'_, T, B>
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
			B::unblock (self.0 as *const _ as usize);
		}
	}
}

#[derive(Debug)]
pub struct RWFutexImplWriteGuard<'a, T, B: Blocker> (&'a RWFutexImpl<T, B>);

impl <'a, T, B: Blocker> RWFutexImplWriteGuard<'a, T, B>
{
	pub fn new (futex: &'a RWFutexImpl<T, B>) -> Self
	{
		RWFutexImplWriteGuard(futex) 
	}
}

impl<T, B: Blocker> Deref for RWFutexImplWriteGuard<'_, T, B>
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

impl<T, B: Blocker> DerefMut for RWFutexImplWriteGuard<'_, T, B>
{
	fn deref_mut (&mut self) -> &mut Self::Target
	{
		unsafe
		{
			self.0.data.get ().as_mut ().unwrap ()
		}
	}
}

impl<T, B: Blocker> Drop for RWFutexImplWriteGuard<'_, T, B>
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
			B::unblock (self.0 as *const _ as usize);
		}
	}
}
