use crate::uses::*;
use core::ops::Range;

// must be power of 2 for correct results
pub const fn align_up (addr: usize, align: usize) -> usize
{
	(addr + align - 1) & !(align - 1)
}

// must be power of 2 for correct results
pub const fn align_down (addr: usize, align: usize) -> usize
{
	addr & !(align - 1)
}

pub fn align_of (addr: usize) -> usize
{
	if addr == 0
	{
		return 1 << 63;
	}

	let out: usize;

	unsafe
	{
		asm!("bsf {}, {}",
			out(reg) out,
			in(reg) addr);
	}

	1 << out
}

pub const fn get_bits (n: usize, bits: Range<usize>) -> usize
{
	if bits.end == 0
	{
		return 0;
	}

	let l = if bits.start > 63 { 63 } else { bits.start };
	let h = if bits.end > 64 { 63 } else { bits.end - 1 };
	if l >= h
	{
		return 0;
	}

	let temp = if h == 63
	{
		usize::MAX
	}
	else
	{
		(1 << (h + 1)) - 1
	};

	(temp & n).wrapping_shr (l as _)
}

pub const fn get_bits_raw (n: usize, bits: Range<usize>) -> usize
{
	let l = if bits.start > 63 { 63 } else { bits.start };
	let h = if bits.end > 63 { 63 } else { bits.end };
	if l >= h
	{
		return 0;
	}
	
	let temp = if h == 63
	{
		usize::MAX
	}
	else
	{
		(1 << (h + 1)) - 1
	};

	(temp & n).wrapping_shr (l as _) << l
}

pub unsafe fn memset (mem: *mut u8, len: usize, data: u8)
{
	for i in 0..len
	{
		*mem.add (i) = data;
	}
}

// rounds down
#[inline]
pub fn log2 (n: usize) -> usize
{
	if n == 0
	{
		return 0;
	}

	let out;

	unsafe
	{
		asm!("bsr {}, {}",
			out(reg) out,
			in(reg) n);
	}

	out
}

// rounds up
// TODO: make faster
pub fn log2_up (n: usize) -> usize
{
	if n == 1
	{
		1
	}
	else
	{
		log2 (align_up (n, 1 << log2 (n)))
	}
}

pub const fn log2_const (n: usize) -> usize
{
	if n == 0
	{
		return 0;
	}

	let mut out = 0;
	while get_bits (n, out..64) > 0
	{
		out += 1;
	}

	out - 1
}

pub const fn log2_up_const (n: usize) -> usize
{
	if n == 1
	{
		1
	}
	else
	{
		log2_const (align_up (n, 1 << log2_const (n)))
	}
}

pub unsafe fn unbound<'a, 'b, T> (r: &'a T) -> &'b T
{
	(r as *const T).as_ref ().unwrap ()
}

pub unsafe fn unbound_mut<'a, 'b, T> (r: &'a mut T) -> &'b mut T
{
	(r as *mut T).as_mut ().unwrap ()
}
