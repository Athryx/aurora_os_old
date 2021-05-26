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
