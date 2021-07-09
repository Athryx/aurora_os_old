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
	OutOfMem = 1,
	InvlVirtMem = 2,
	InvlPtr = 3,
	InvlVirtAddr = 4,
	InvlArgs = 5,
	InvlPriv = 6,
	Unknown = 7,
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
			Self::OutOfMem => "out of memory",
			Self::InvlVirtMem => "virtual memory collision",
			Self::InvlPtr => "invalid pointer",
			Self::InvlVirtAddr => "nan canonical pointer",
			Self::InvlArgs => "invalid arguments",
			Self::InvlPriv => "insufficent priveledge",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MsgErr
{
	Recieve = 0,
	NonBlockOk = 1,
	BlockOkResp = 2,
	BlockOkRet = 3,
	BlockOkTerm = 4,
	InvlId = 5,
	InvlPriv = 6,
	InvlReply = 7,
	Unknown = 8,
}

impl MsgErr
{
	pub fn new (n: u8) -> Option<Self>
	{
		if n > Self::Unknown as u8
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

	pub fn num (&self) -> u8
	{
		*self as u8
	}

	pub fn as_str (&self) -> &'static str
	{
		match self
		{
			Self::Recieve => "message recieved",
			Self::NonBlockOk => "non blocking message succesfully sent",
			Self::BlockOkResp => "blocking message sent, and a response was recieved",
			Self::BlockOkRet => "blocking message sent, and the recipipient called msg_return",
			Self::BlockOkTerm => "blocking message sent, and the recipipient thread terminated",
			Self::InvlId => "no process with specified pid or eid exists",
			Self::InvlPriv => "insufficent priveledges to send a message to the specified process and domain",
			Self::InvlReply => "thread tried to reply but it did not have an open connection",
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
