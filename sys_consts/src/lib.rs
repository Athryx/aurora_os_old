//! Crate for constants related to epoch kernel system calls
#![no_std]

pub mod options;
pub mod syscalls;

/// Error codes returned by syscalls
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum SysErr
{
	Ok = 0,
	OkUnreach = 1,
	OutOfMem = 2,
	InvlVirtMem = 3,
	InvlPtr = 4,
	InvlVirtAddr = 5,
	InvlArgs = 6,
	InvlId = 7,
	InvlPriv = 8,
	InvlCap = 9,
	InvlString = 11,
	InvlOp = 12,
	Obscured = 13,
	Unknown = 14,
}

impl SysErr
{
	pub fn new(n: usize) -> Option<Self>
	{
		if n > Self::Unknown as usize {
			None
		} else {
			unsafe { Some(core::mem::transmute(n)) }
		}
	}

	pub const fn num(&self) -> usize
	{
		*self as usize
	}

	pub const fn as_str(&self) -> &'static str
	{
		match self {
			Self::Ok => "no error",
			Self::OkUnreach => "ipc message not sent or recieved because there was no waiting thread",
			Self::OutOfMem => "out of memory",
			Self::InvlVirtMem => "virtual memory collision",
			Self::InvlPtr => "invalid pointer",
			Self::InvlVirtAddr => "non canonical pointer",
			Self::InvlArgs => "invalid arguments",
			Self::InvlId => "invalid identifier",
			Self::InvlPriv => "insufficent priveledge",
			Self::InvlCap => "invalid capability permissions",
			Self::InvlString => "invalid utf-8 string",
			Self::InvlOp => "invalid operation",
			Self::Obscured => "operation does not return information about error state",
			Self::Unknown => "unknown error",
		}
	}
}

pub mod thread
{
	pub const YIELD: usize = 0;
	pub const DESTROY: usize = 1;
	pub const SLEEP: usize = 2;
	pub const JOIN: usize = 3;
}
