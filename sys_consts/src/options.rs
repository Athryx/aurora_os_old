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

bitflags!
{
	pub struct MsgOptions: u32
	{
		const PID = 1;
		const BLOCK = 1 << 1;
		const SMEM1 = 1 << 4;
		const SMEM2 = 1 << 5;
		const SMEM3 = 1 << 6;
		const SMEM4 = 1 << 7;
	}
}
