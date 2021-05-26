#![no_std]
//! Crate for constants related to epoch kernel system calls

pub mod syscalls;
pub mod options;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum SysErr
{
	Ok = 0,
	OutOfMem = 1,
	AllocAtInvlAddr = 2,
	InvlPointer = 3,
	InvlVirtAddr = 4,
	Unknown = 5,
}

impl SysErr
{
	pub fn new (n: usize) -> Option<Self>
	{
		if n > Self::Unknown as usize
		{
			None
		}
		else
		{
			unsafe
			{
				Some(core::mem::transmute (n))
			}
		}
	}

	pub fn num (&self) -> usize
	{
		*self as usize
	}
}

pub mod thread
{
	pub const YIELD: usize = 0;
	pub const DESTROY: usize = 1;
	pub const SLEEP: usize = 2;
	pub const JOIN: usize = 3;
}
