//! Crate for constants related to epoch kernel system calls
#![no_std]

#![feature(try_trait)]

use core::option::NoneError;

pub mod syscalls;
pub mod options;

/// Error codes returned by syscalls
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum SysErr
{
	Ok = 0,
	MsgResp = 1,
	MsgRet = 2,
	MsgTerm = 3,
	OutOfMem = 4,
	InvlVirtMem = 5,
	InvlPtr = 6,
	InvlVirtAddr = 7,
	InvlArgs = 8,
	InvlPriv = 9,
	InvlId = 10,
	Unknown = 11,
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

	pub fn as_str (&self) -> &'static str
	{
		match self
		{
			Self::Ok => "no error",
			Self::MsgResp => "blocking message sent, and a response was recieved",
			Self::MsgRet => "blocking message sent, and the recipipient called msg_return",
			Self::MsgTerm => "blocking message sent, and the recipipient thread terminated",
			Self::OutOfMem => "out of memory",
			Self::InvlVirtMem => "virtual memory collision",
			Self::InvlPtr => "invalid pointer",
			Self::InvlVirtAddr => "nan canonical pointer",
			Self::InvlArgs => "invalid arguments",
			Self::InvlPriv => "insufficent priveledge",
			Self::InvlId => "invalid identifier",
			Self::Unknown => "unknown error",
		}
	}
}

impl From<NoneError> for SysErr
{
	fn from (_: NoneError) -> Self
	{
		SysErr::Unknown
	}
}

pub mod thread
{
	pub const YIELD: usize = 0;
	pub const DESTROY: usize = 1;
	pub const SLEEP: usize = 2;
	pub const JOIN: usize = 3;
}
