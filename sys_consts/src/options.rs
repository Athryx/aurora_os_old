//! options for epoch kernel syscalls
use bitflags::bitflags;

bitflags!
{
	pub struct ReallocOptions: u32
	{
		const READ = 1;
		const WRITE = 1 << 1;
		const EXEC = 1 << 2;
		const EXACT = 1 << 4;
	}
}

bitflags!
{
	pub struct RegOptions: u32
	{
		const BLOCK = 1;
		const DEFAULT = 1 << 1;
		const PUBLIC = 1 << 2;
		const GROUP = 1 << 3;
	}
}
