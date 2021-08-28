use crate::uses::*;
use core::alloc::{GlobalAlloc, Layout};
use core::cmp::min;
use x86_64::VirtAddr;
use crate::futex::Futex;
use heap::LinkedListAllocator;

pub mod heap;

#[derive(Debug, Clone, Copy)]
pub struct Allocation
{
	ptr: VirtAddr,
	len: usize,
	pub zindex: usize,
}

impl Allocation
{
	// NOTE: panics if addr is not canonical
	pub fn new (addr: usize, len: usize) -> Self
	{
		Allocation {
			ptr: VirtAddr::new (addr as _),
			len,
			zindex: 0,
		}
	}

	pub fn addr (&self) -> VirtAddr
	{
		self.ptr
	}

	pub fn as_mut_ptr<T> (&mut self) -> *mut T
	{
		self.ptr.as_mut_ptr ()
	}

	pub fn as_ptr<T> (&self) -> *const T
	{
		self.ptr.as_ptr ()
	}

	pub fn as_slice (&self) -> &[u8]
	{
		unsafe { core::slice::from_raw_parts (self.as_ptr (), self.len) }
	}

	pub fn as_mut_slice (&mut self) -> &mut [u8]
	{
		unsafe { core::slice::from_raw_parts_mut (self.as_mut_ptr (), self.len) }
	}

	pub fn as_usize (&self) -> usize
	{
		self.ptr.as_u64 () as usize
	}

	pub fn len (&self) -> usize
	{
		self.len
	}

	// returns number of bytes copied
	pub fn copy_from_mem (&mut self, other: &Self) -> usize
	{
		let size = min (self.len (), other.len ());
		unsafe
		{
			let dst: &mut [u8] = core::slice::from_raw_parts_mut (self.as_mut_ptr (), size);
			let src: &[u8] = core::slice::from_raw_parts (other.as_ptr (), size);
			dst.copy_from_slice (src);
		}
		size
	}
}

#[global_allocator]
static ALLOCATOR: GlobalAllocator = GlobalAllocator::new ();

#[alloc_error_handler]
fn alloc_error_handler (layout: Layout) -> !
{
	panic! ("allocation error: {:?}", layout);
}

pub fn init ()
{
	ALLOCATOR.init ();
}

// TODO: add relloc function
struct GlobalAllocator
{
	allocer: Futex<Option<LinkedListAllocator>>,
}

impl GlobalAllocator
{
	const fn new () -> GlobalAllocator
	{
		GlobalAllocator {
			allocer: Futex::new (None),
		}
	}

	fn init (&self)
	{
		*self.allocer.lock () = Some(LinkedListAllocator::new ());
	}
}

unsafe impl GlobalAlloc for GlobalAllocator
{
	unsafe fn alloc (&self, layout: Layout) -> *mut u8
	{
		self.allocer.lock ().as_mut ().unwrap ().alloc (layout)
	}

	unsafe fn dealloc (&self, ptr: *mut u8, layout: Layout)
	{
		self.allocer.lock ().as_mut ().unwrap ().dealloc (ptr, layout)
	}
}

unsafe impl Send for GlobalAllocator {}
unsafe impl Sync for GlobalAllocator {}
