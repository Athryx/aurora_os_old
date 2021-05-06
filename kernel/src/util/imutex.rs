use core::ops::{Deref, DerefMut};
use spin::{Mutex, MutexGuard};
use crate::arch::x64::{cli_safe, sti_safe};

// A Mutex that also disables interrupts when locked
#[derive(Debug)]
pub struct IMutex<T: ?Sized>(Mutex<T>);

impl<T> IMutex<T>
{
	pub const fn new (user_data: T) -> Self
	{
		IMutex(Mutex::new (user_data))
	}

	pub fn into_inner (self) -> T
	{
		self.0.into_inner ()
	}

	pub fn lock (&self) -> IMutexGuard<T>
	{
		cli_safe ();
		IMutexGuard(self.0.lock ())
	}

	pub fn try_lock (&self) -> Option<IMutexGuard<T>>
	{
		self.0.try_lock ().map (|guard| {
			IMutexGuard(guard)
		})
	}

	pub unsafe fn force_unlock (&self)
	{
		self.0.force_unlock ();
	}
}

impl<T: ?Sized + Default> Default for IMutex<T>
{
	fn default () -> IMutex<T>
	{
		IMutex::new (Default::default ())
	}
}

unsafe impl<T: ?Sized + Send> Send for IMutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for IMutex<T> {}

pub struct IMutexGuard<'a, T: ?Sized + 'a>(MutexGuard<'a, T>);

impl<T> Deref for IMutexGuard<'_, T>
{
	type Target = T;

	fn deref (&self) -> &Self::Target
	{
		&self.0
	}
}

impl<T> DerefMut for IMutexGuard<'_, T>
{
	fn deref_mut (&mut self) -> &mut Self::Target
	{
		&mut self.0
	}
}

impl<T: ?Sized> Drop for IMutexGuard<'_, T>
{
	fn drop (&mut self)
	{
		sti_safe ();
	}
}
