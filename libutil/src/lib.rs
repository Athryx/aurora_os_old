//! Basic library that has code that is shared between userspace and kernel
#![no_std]
#![feature(asm)]
#![feature(allocator_api)]
#![feature(alloc_prelude)]
#![feature(alloc_error_handler)]
#![feature(const_fn_trait_bound)]

extern crate alloc;

pub mod atomic;
pub mod cell;
pub mod collections;
pub mod futex;
pub mod mem;
pub mod misc;
pub mod ptr;

mod uses;

use mem::Allocation;

static mut UTIL_CALLS: Option<&'static dyn UtilCalls> = None;

pub trait UtilCalls
{
	fn futex_new(&self) -> usize;
	fn futex_destroy(&self, id: usize);
	fn block(&self, id: usize);
	fn unblock(&self, id: usize);

	fn alloc(&self, size: usize) -> Option<Allocation>;
	fn dealloc(&self, mem: Allocation);
}

fn futex_new() -> usize
{
	unsafe {
		UTIL_CALLS.as_ref().unwrap().futex_new()
	}
}

fn futex_destroy(id: usize)
{
	unsafe {
		UTIL_CALLS.as_ref().unwrap().futex_destroy(id)
	}
}

fn block(id: usize)
{
	unsafe {
		UTIL_CALLS.as_ref().unwrap().block(id);
	}
}

fn unblock(id: usize)
{
	unsafe {
		UTIL_CALLS.as_ref().unwrap().unblock(id);
	}
}

fn alloc(size: usize) -> Option<Allocation>
{
	unsafe { UTIL_CALLS.as_ref().unwrap().alloc(size) }
}

fn dealloc(mem: Allocation)
{
	unsafe {
		UTIL_CALLS.as_ref().unwrap().dealloc(mem);
	}
}

/// safety: can only be called singel threaded, and cannot call any other library functions untill after this returns
pub unsafe fn init(calls: &'static dyn UtilCalls)
{
	UTIL_CALLS = Some(calls);

	#[cfg(not(feature = "kernel"))]
	mem::init();
}
