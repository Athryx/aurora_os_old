use crate::uses::*;
use core::ops::Range;

// mutex is too slow
static mut MEM_OFFSET: u64 = 0; 

// panics if PhysAddr is too big
pub fn phys_to_virt (paddr: PhysAddr) -> VirtAddr
{
	VirtAddr::try_new (paddr.as_u64 () + unsafe { MEM_OFFSET })
		.unwrap_or_else (|_| {
			panic! ("physical address was too big to convert to virtual address");
		})
}

pub fn virt_to_phys (vaddr: VirtAddr) -> PhysAddr
{
	let a = get_bits (vaddr.as_u64 () as _, 0..48) as u64;
	if a < unsafe { MEM_OFFSET }
	{
		panic! ("virtual address was too small to convert to physical address");
	}
	// TODO: handle case when VirtAddr is bigger than meximum physical memory available
	PhysAddr::new (a - unsafe { MEM_OFFSET })
}

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
		return 0;
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
	log2 (align_up (n, 1 << log2 (n)))
}

pub fn init (mem_offset: u64)
{
	unsafe
	{
		MEM_OFFSET = mem_offset;
	}
}
