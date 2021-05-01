pub const KERNEL_UID: usize = 0;
pub const SUPERUSER_UID: usize = 1;
pub const IOPRIV_UID: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PrivLevel
{
	Kernel,
	SuperUser,
	IOPriv,
	User(usize),
}

impl PrivLevel
{
	pub fn new (uid: usize) -> Self
	{
		match uid
		{
			KERNEL_UID => Self::Kernel,
			SUPERUSER_UID => Self::SuperUser,
			IOPRIV_UID => Self::IOPriv,
			_ => Self::User(uid),
		}
	}

	pub fn uid (&self) -> usize
	{
		match self
		{
			Self::Kernel => KERNEL_UID,
			Self::SuperUser => SUPERUSER_UID,
			Self::IOPriv => IOPRIV_UID,
			Self::User(uid) => *uid,
		}
	}
}
