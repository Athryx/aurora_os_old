use crate::uses::*;
use crate::mem::{VirtRange, PAGE_SIZE};
use crate::sched::proc_c;
use crate::consts::KERNEL_VMA;

// this trait represents data structures that can be fetched from user controlled memory by syscalls
// safety: because the user controls the memory, the structre shold be defined for all bit patterns
// so mostly structures containing only integers, and no enums
pub unsafe trait UserData: Copy
{
}

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

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct UserPageArray
{
	addr: usize,
	// len in pages
	len: usize,
}

impl UserPageArray
{
	pub fn from_parts(addr: usize, len: usize) -> Self
	{
		UserPageArray {
			addr,
			len,
		}
	}

	pub fn addr(&self) -> usize
	{
		self.addr
	}

	pub fn byte_len(&self) -> usize
	{
		self.len * PAGE_SIZE
	}

	pub fn page_len(&self) -> usize
	{
		self.len
	}

	pub fn verify(&self) -> bool
	{
		align_of(self.addr) >= PAGE_SIZE && verify_umem(self.addr, self.len * PAGE_SIZE)
	}

	pub fn as_virt_zone(&self) -> Result<VirtRange, SysErr>
	{
		VirtRange::try_new_user(self.addr, self.len * PAGE_SIZE)
	}
}

unsafe impl UserData for UserPageArray {}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct UserArray<T: UserData>
{
	ptr: *const T,
	len: usize,
}

impl<T: UserData + Default> UserArray<T>
{
	pub fn from_parts(ptr: *const T, len: usize) -> Self
	{
		UserArray {
			ptr,
			len,
		}
	}

	pub fn ptr(&self) -> *const T
	{
		self.ptr
	}

	pub fn len(&self) -> usize
	{
		self.len
	}

	pub fn try_fetch(&self) -> Option<Vec<T>>
	{
		if !aligned_nonnull(self.ptr) {
			return None;
		}

		let range = VirtRange::new_unaligned(
			VirtAddr::try_new(self.ptr as u64).ok()?,
			self.len * size_of::<T>(),
		);
		if !range.verify_umem() {
			return None;
		}

		proc_c().addr_space.range_map(range, |data| {
			let slice = unsafe { core::slice::from_raw_parts(data.as_ptr() as *const T, self.len) };
			Some(copy_to_heap(slice))
		})
	}
}

impl<T: UserData> Default for UserArray<T>
{
	fn default() -> Self
	{
		UserArray {
			ptr: null(),
			len: 0,
		}
	}
}

unsafe impl<T: UserData> UserData for UserArray<T> {}

// TODO: decide if UserString is even necessary, or if UserArray is enough
#[derive(Debug, Clone, Copy, Default)]
#[repr(transparent)]
pub struct UserString
{
	data: UserArray<u8>,
}

impl UserString
{
	pub fn from_parts(ptr: *const u8, len: usize) -> Self
	{
		UserString {
			data: UserArray::from_parts(ptr, len),
		}
	}

	pub fn ptr(&self) -> *const u8
	{
		self.data.ptr()
	}

	pub fn len(&self) -> usize
	{
		self.data.len()
	}

	pub fn try_fetch(&self) -> Option<String>
	{
		String::from_utf8(self.data.try_fetch()?).ok()
	}
}

unsafe impl UserData for UserString {}

pub fn fetch_data<T: UserData>(ptr: *const T) -> Option<T>
{
	if !aligned_nonnull(ptr) {
		return None;
	}

	let range = VirtRange::new_unaligned(VirtAddr::try_new(ptr as u64).ok()?, size_of::<T>());
	if !range.verify_umem() {
		return None;
	}

	proc_c().addr_space.range_map(range, |data| unsafe {
		Some(ptr::read_unaligned(data.as_ptr() as *const T))
	})
}

pub fn verify_uaddr(addr: usize) -> bool
{
	addr < *KERNEL_VMA
}

pub fn verify_umem(addr: usize, size: usize) -> bool
{
	verify_uaddr(addr + size - 1)
}
