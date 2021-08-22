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
		const GLOBAL = 1 << 2;
		const PUBLIC = 1 << 3;
		const REMOVE = 1 << 4;
		const GROUP = 1 << 5;
	}
}

bitflags!
{
	pub struct ConnectOptions: u32
	{
		const PID = 1;
	}
}

bitflags!
{
	pub struct MsgOptions: u32
	{
		const PID = 1;
		const BLOCK = 1 << 1;
		const REPLY = 1 << 2;
		const SMEM1 = 1 << 8;
		const SMEM2 = 1 << 9;
		const SMEM3 = 1 << 10;
		const SMEM4 = 1 << 11;
		const SMEM5 = 1 << 12;
		const SMEM6 = 1 << 13;
		const SMEM7 = 1 << 14;
		const SMEM8 = 1 << 15;
	}
}
