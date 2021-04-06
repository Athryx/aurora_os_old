use core::ops::Range;

// must be power of 2 for correct results
pub const fn align_up (addr: usize, align: usize) -> usize
{
	(addr + align - 1) & !(align - 1)
}

// must be power of 2 for correct results
pub const fn align_dowm (addr: usize, align: usize) -> usize
{
	addr & !(align - 1)
}

pub const fn align_of (addr: usize) -> usize
{
	if addr > 0
	{
		(addr ^ (addr - 1)) + 1
	}
	else
	{
		0
	}
}

pub const fn get_bits (n: usize, bits: Range<usize>) -> usize
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
