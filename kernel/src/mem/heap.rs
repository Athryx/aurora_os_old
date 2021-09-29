use alloc::alloc::{GlobalAlloc, Layout};

use libutil::mem::heap::LinkedListAllocator;

use crate::uses::*;
use crate::util::IMutex;

#[global_allocator]
static ALLOCATOR: GlobalAllocator = GlobalAllocator::new();

pub fn init()
{
	ALLOCATOR.init();
}

// TODO: add relloc function
struct GlobalAllocator
{
	allocer: IMutex<Option<LinkedListAllocator>>,
}

impl GlobalAllocator
{
	const fn new() -> GlobalAllocator
	{
		GlobalAllocator {
			allocer: IMutex::new(None),
		}
	}

	fn init(&self)
	{
		*self.allocer.lock() = Some(LinkedListAllocator::new());
	}
}

unsafe impl GlobalAlloc for GlobalAllocator
{
	unsafe fn alloc(&self, layout: Layout) -> *mut u8
	{
		self.allocer.lock().as_mut().unwrap().alloc(layout)
	}

	unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout)
	{
		self.allocer.lock().as_mut().unwrap().dealloc(ptr, layout)
	}
}

unsafe impl Send for GlobalAllocator {}
unsafe impl Sync for GlobalAllocator {}
