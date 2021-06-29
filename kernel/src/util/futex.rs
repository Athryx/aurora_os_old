use crate::uses::*;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use core::ops::{Deref, DerefMut};
use core::cell::UnsafeCell;
use crate::sched::*;

pub struct Futex<T>
{
	acquired: AtomicBool,
	waiting: AtomicUsize,
	data: UnsafeCell<T>,
}

impl<T> Futex<T>
{
	pub fn new (data: T) -> Self
	{
		Futex {
			acquired: AtomicBool::new (false),
			waiting: AtomicUsize::new (0),
			data: UnsafeCell::new (data),
		}
	}

	pub fn lock (&self) -> FutexGaurd<T>
	{
		loop
		{
			match self.try_lock ()
			{
				Ok(guard) => return guard,
				Err(_) => {
					self.waiting.fetch_add (1, Ordering::Relaxed);
					thread_c ().block (ThreadState::FutexBlock(self as *const _ as usize));
				},
			}
		}
	}

	pub fn try_lock (&self) -> Result<FutexGaurd<T>, ()>
	{
		let acq = self.acquired.swap (true, Ordering::Relaxed);
		if acq
		{
			Err(())
		}
		else
		{
			Ok(FutexGaurd::new (self))
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

pub struct FutexGaurd<'a, T> (&'a Futex<T>);

impl <'a, T> FutexGaurd<'a, T>
{
	pub fn new (futex: &'a Futex<T>) -> Self
	{
		FutexGaurd(futex) 
	}
}

impl<T> Deref for FutexGaurd<'_, T>
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

impl<T> DerefMut for FutexGaurd<'_, T>
{
	fn deref_mut (&mut self) -> &mut Self::Target
	{
		unsafe
		{
			self.0.data.get ().as_mut ().unwrap ()
		}
	}
}

impl<T> Drop for FutexGaurd<'_, T>
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
			proc_c ().futex_move (self.0 as *const _ as usize, ThreadState::Ready, 1);
		}
	}
}
