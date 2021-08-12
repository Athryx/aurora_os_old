use crate::uses::*;
use crate::mem::VirtRange;
use crate::util::{copy_to_heap, aligned_nonnull};
use crate::sched::proc_c;

// this trait represents data structures that can be fetched from user controlled memory by syscalls
// safety: because the user controls the memory, the structre shold be defined for all bit patterns
// so mostly structures containing only integers, and no enums
pub unsafe trait UserData: Copy {}

unsafe impl UserData for u8 {}
unsafe impl UserData for u16 {}
unsafe impl UserData for u32 {}
unsafe impl UserData for u64 {}
unsafe impl UserData for usize {}

unsafe impl UserData for i8 {}
unsafe impl UserData for i16 {}
unsafe impl UserData for i32 {}
unsafe impl UserData for i64 {}
unsafe impl UserData for isize {}

#[derive(Debug, Clone, Copy)]
pub struct UserArray<T: UserData>
{
	ptr: *const T,
	len: usize,
}

impl<T: UserData + Default> UserArray<T>
{
	pub fn from_parts (ptr: *const T, len: usize) -> Self
	{
		UserArray {
			ptr,
			len,
		}
	}

	pub fn ptr (&self) -> *const T
	{
		self.ptr
	}

	pub fn len (&self) -> usize
	{
		self.len
	}

	pub fn try_fetch (&self) -> Option<Vec<T>>
	{
		if !aligned_nonnull (self.ptr)
		{
			return None;
		}

		let range = VirtRange::new_unaligned (VirtAddr::try_new (self.ptr as u64).ok ()?, self.len * size_of::<T> ());
		proc_c ().addr_space.range_map (range, |data| {
			let slice = unsafe { core::slice::from_raw_parts (data.as_ptr () as *const T, self.len) };
			Some(copy_to_heap (slice))
		})
	}
}

impl<T: UserData> Default for UserArray<T>
{
	fn default () -> Self
	{
		UserArray {
			ptr: null (),
			len: 0,
		}
	}
}

unsafe impl<T: UserData> UserData for UserArray<T> {}

// TODO: decide if UserString is even necessary, or if UserArray is enough
#[derive(Debug, Clone, Copy)]
pub struct UserString
{
	data: UserArray<u8>
}

impl UserString
{
	pub fn ptr (&self) -> *const u8
	{
		self.data.ptr ()
	}

	pub fn len (&self) -> usize
	{
		self.data.len ()
	}

	pub fn try_fetch (&self) -> Option<String>
	{
		String::from_utf8 (self.data.try_fetch ()?).ok ()
	}
}

impl Default for UserString
{
	fn default () -> Self
	{
		UserString {
			data: UserArray::default (),
		}
	}
}

unsafe impl UserData for UserString {}

pub fn fetch_data<T: UserData> (ptr: *const T) -> Option<T>
{
	if !aligned_nonnull (ptr)
	{
		return None;
	}

	let range = VirtRange::new_unaligned (VirtAddr::try_new (ptr as u64).ok ()?, size_of::<T> ());
	proc_c ().addr_space.range_map (range, |data| {
		unsafe
		{
			Some(ptr::read_unaligned (data.as_ptr () as *const T))
		}
	})
}
