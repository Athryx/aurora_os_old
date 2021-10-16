use crate::uses::*;

// TODO: in futrue, make a macro to generate these enums that are just subsets of the SysErr enum
#[derive(Debug, Clone, Copy)]
pub enum MemErr
{
	OutOfMem(&'static str),
	InvlVirtMem(&'static str),
	InvlPtr(&'static str),
	InvlArgs(&'static str),
	InvlMemType(&'static str),
}

impl MemErr
{
	pub fn as_str(&self) -> &'static str
	{
		match self {
			Self::OutOfMem(msg) => msg,
			Self::InvlVirtMem(msg) => msg,
			Self::InvlPtr(msg) => msg,
			Self::InvlArgs(msg) => msg,
			Self::InvlMemType(msg) => msg,
		}
	}
}

impl Error for MemErr
{
	fn get_error(&self) -> &str
	{
		self.as_str()
	}
}

impl From<MemErr> for SysErr
{
	fn from(err: MemErr) -> Self
	{
		match err {
			MemErr::OutOfMem(_) => Self::OutOfMem,
			MemErr::InvlVirtMem(_) => Self::InvlMemZone,
			MemErr::InvlPtr(_) => Self::InvlPtr,
			MemErr::InvlArgs(_) => Self::InvlArgs,
			MemErr::InvlMemType(_) => Self::InvlArgs,
		}
	}
}

impl From<MemErr> for Err
{
	fn from(err: MemErr) -> Self
	{
		Self::new(err.as_str())
	}
}
