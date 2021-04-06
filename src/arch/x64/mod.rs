use crate::util::misc::*;

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum PrivLevel
{
	Ring0 = 0,
	Ring3 = 3,
}

impl PrivLevel
{
	pub const fn n (&self) -> u8
	{
		*self as u8
	}

	pub const fn get_cs (&self) -> u16
	{
		match self
		{
			Self::Ring0 => 0x8,
			Self::Ring3 => 0x23,
		}
	}

	pub const fn get_ds (&self) -> u16
	{
		match self
		{
			Self::Ring0 => 0x10,
			Self::Ring3 => 0x1b,
		}
	}
}

pub const FSBASE_MSR: u32 = 0xc0000100;
pub const GSBASE_MSR: u32 = 0xc0000101;
pub const GSBASEK_MSR: u32 = 0xc0000102;

#[inline]
pub fn rdmsr (msr: u32) -> u64
{
	let low: u32;
	let high: u32;
	unsafe
	{
		asm! ("rdmsr", in("ecx") msr, out("eax") low, out("edx") high, options(nomem, nostack));
	}
	((high as u64) << 32) | low as u64
}

#[inline]
pub fn wrmsr (msr: u32, data: u64)
{
	let low = get_bits (data as usize, 0..32);
	let high = get_bits (data as usize, 32..64);
	unsafe
	{
		asm! ("wrmsr", in("ecx") msr, in("eax") low, in("edx") high, options(nomem, nostack));
	}
}

#[inline]
pub fn hlt ()
{
	unsafe
	{
		asm!("hlt", options(nomem, nostack));
	}
}

#[inline]
pub fn cli ()
{
	unsafe
	{
		asm!("cli", options(nomem, nostack));
	}
}

#[inline]
pub fn sti ()
{
	unsafe
	{
		asm!("sti", options(nomem, nostack));
	}
}

#[inline]
pub fn outb (port: u16, data: u8)
{
	unsafe
	{
		asm!("out dx, al", in("dx") port, in("al") data);
	}
}

#[inline]
pub fn outw (port: u16, data: u16)
{
	unsafe
	{
		asm!("out dx, al", in("dx") port, in("ax") data);
	}
}

#[inline]
pub fn outd (port: u16, data: u32)
{
	unsafe
	{
		asm!("out dx, al", in("dx") port, in("eax") data);
	}
}

#[inline]
pub fn inb (port: u16) -> u8
{
	let out;
	unsafe
	{
		asm!("in al, dx", in("dx") port, out("al") out);
	}
	out
}

#[inline]
pub fn inw (port: u16) -> u16
{
	let out;
	unsafe
	{
		asm!("in ax, dx", in("dx") port, out("ax") out);
	}
	out
}

#[inline]
pub fn ind (port: u16) -> u32
{
	let out;
	unsafe
	{
		asm!("in eax, dx", in("dx") port, out("eax") out);
	}
	out
}

#[inline]
pub fn get_cr0 () -> usize
{
	let out;
	unsafe
	{
		asm!("mov cr0, {}", out(reg) out, options(nomem, nostack));
	}
	out
}

#[inline]
pub fn set_cr0 (n: usize)
{
	unsafe
	{
		asm!("mov {}, cr0", in(reg) n, options(nomem, nostack));
	}
}

#[inline]
pub fn get_cr2 () -> usize
{
	let out;
	unsafe
	{
		asm!("mov cr2, {}", out(reg) out, options(nomem, nostack));
	}
	out
}

#[inline]
pub fn set_cr2 (n: usize)
{
	unsafe
	{
		asm!("mov {}, cr2", in(reg) n, options(nomem, nostack));
	}
}

#[inline]
pub fn get_cr3 () -> usize
{
	let out;
	unsafe
	{
		asm!("mov cr3, {}", out(reg) out, options(nomem, nostack));
	}
	out
}

#[inline]
pub fn set_cr3 (n: usize)
{
	unsafe
	{
		asm!("mov {}, cr3", in(reg) n, options(nomem, nostack));
	}
}

#[inline]
pub fn get_cr4 () -> usize
{
	let out;
	unsafe
	{
		asm!("mov cr4, {}", out(reg) out, options(nomem, nostack));
	}
	out
}

#[inline]
pub fn set_cr4 (n: usize)
{
	unsafe
	{
		asm!("mov {}, cr4", in(reg) n, options(nomem, nostack));
	}
}
