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
	MsgResp = 1,
	MsgUnreach = 2,
	MsgTerm = 3,
	OutOfMem = 4,
	InvlVirtMem = 5,
	InvlPtr = 6,
	InvlVirtAddr = 7,
	InvlArgs = 8,
	InvlPriv = 9,
	InvlId = 10,
	InvlString = 11,
	InvlOp = 12,
	Unknown = 13,
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
			Self::MsgResp => "blocking message sent, and a response was recieved",
			Self::MsgUnreach => {
				"cannot send message, no waiting thread or registered domain handler"
			},
			Self::MsgTerm => "cannot send message, connection terminated",
			Self::OutOfMem => "out of memory",
			Self::InvlVirtMem => "virtual memory collision",
			Self::InvlPtr => "invalid pointer",
			Self::InvlVirtAddr => "non canonical pointer",
			Self::InvlArgs => "invalid arguments",
			Self::InvlPriv => "insufficent priveledge",
			Self::InvlId => "invalid identifier",
			Self::InvlString => "invalid utf-8 string",
			Self::InvlOp => "invalid operation",
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
