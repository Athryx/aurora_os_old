use core::cmp::min;
use core::marker::PhantomData;
use crate::uses::*;
use crate::mb2::BootInfo;

// unless otherwise stated, all lens in this module are in bytes, not pages
// TODO: make traits or macros to reduce duplicated code on Phys and Virt versions of all these types

pub mod phys_alloc;
pub mod virt_alloc;
pub mod kernel_heap;
pub mod sys;

pub const PAGE_SIZE: usize = 4096;
pub const MAX_VIRT_ADDR: usize = 1 << 47;

pub fn align_down_to_page_size (n: usize) -> usize
{
	if n >= PageSize::G1 as usize
	{
		PageSize::G1 as usize
	}
	else if n >= PageSize::M2 as usize
	{
		PageSize::M2 as usize
	}
	else if n >= PageSize::K4 as usize
	{
		PageSize::K4 as usize
	}
	else
	{
		0
	}
}

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PageSize
{
	K4 = 0x1000,
	M2 = 0x200000,
	G1 = 0x40000000,
}

impl PageSize
{
	pub fn from_u64 (n: u64) -> Self
	{
		Self::from_usize (n as _)
	}

	pub fn from_usize (n: usize) -> Self
	{
		Self::try_from_usize (n).expect ("tried to convert integer to PageSize, but it wasn't a valid page size")
	}

	pub fn try_from_usize (n: usize) -> Option<Self>
	{
		match n
		{
			0x1000 => Some(Self::K4),
			0x200000 => Some(Self::M2),
			0x40000000 => Some(Self::G1),
			_ => None,
		}
	}
}

#[derive(Debug, Clone, Copy)]
pub enum PhysFrame
{
	K4(PhysAddr),
	M2(PhysAddr),
	G1(PhysAddr),
}

impl PhysFrame
{
	pub fn new (addr: PhysAddr, size: PageSize) -> Self
	{
		match size
		{
			PageSize::K4 => Self::K4(addr.align_down (size as u64)),
			PageSize::M2 => Self::M2(addr.align_down (size as u64)),
			PageSize::G1 => Self::G1(addr.align_down (size as u64)),
		}
	}

	pub fn start_addr (&self) -> PhysAddr
	{
		match self
		{
			Self::K4(addr) => *addr,
			Self::M2(addr) => *addr,
			Self::G1(addr) => *addr,
		}
	}

	pub fn end_addr (&self) -> PhysAddr
	{
		match self
		{
			Self::K4(addr) => *addr + PageSize::K4 as u64,
			Self::M2(addr) => *addr + PageSize::M2 as u64,
			Self::G1(addr) => *addr + PageSize::G1 as u64,
		}
	}

	pub fn get_size (&self) -> PageSize
	{
		match self
		{
			Self::K4(_) => PageSize::K4,
			Self::M2(_) => PageSize::M2,
			Self::G1(_) => PageSize::G1,
		}
	}
}

#[derive(Debug, Clone, Copy)]
pub struct PhysRange
{
	addr: PhysAddr,
	size: usize,
}

impl PhysRange
{
	pub fn new (addr: PhysAddr, size: usize) -> Self
	{
		PhysRange {
			addr: addr.align_down (PageSize::K4 as u64),
			size: align_up (size, PageSize::K4 as _),
		}
	}

	pub fn new_unaligned (addr: PhysAddr, size: usize) -> Self
	{
		PhysRange {
			addr,
			size,
		}
	}

	pub fn null () -> Self
	{
		PhysRange {
			addr: PhysAddr::new (0),
			size: 0,
		}
	}

	pub fn aligned (&self) -> PhysRange
	{
		Self::new (self.addr, self.size)
	}

	pub fn addr (&self) -> PhysAddr
	{
		self.addr
	}

	pub fn as_usize (&self) -> usize
	{
		self.addr.as_u64 () as usize
	}

	pub fn end_addr (&self) -> PhysAddr
	{
		self.addr + self.size
	}

	pub fn end_usize (&self) -> usize
	{
		self.as_usize () + self.size
	}

	pub fn contains (&self, addr: PhysAddr) -> bool
	{
		(addr >= self.addr) && (addr < (self.addr + self.size))
	}

	pub fn contains_range (&self, range: Self) -> bool
	{
		self.contains (range.addr ()) || self.contains (range.addr () + range.size ())
	}

	pub fn split_at (&self, range: Self) -> (Option<PhysRange>, Option<PhysRange>)
	{
		let sbegin = self.addr;
		let send = self.addr + self.size;

		let begin = range.addr ();
		let end = begin + range.size ();

		if !self.contains_range (range)
		{
			(Some(*self), None)
		}
		else if begin <= sbegin && end >= send
		{
			(None, None)
		}
		else if self.contains (begin - 1u64) && !self.contains (end + 1u64)
		{
			(Some(PhysRange::new_unaligned (sbegin, (begin - sbegin) as usize)), None)
		}
		else if self.contains (end + 1u64) && !self.contains (begin - 1u64)
		{
			(Some(PhysRange::new_unaligned (end, (send - end) as usize)), None)
		}
		else
		{
			(Some(PhysRange::new_unaligned (sbegin, (begin - sbegin) as usize)),
				Some(PhysRange::new_unaligned (end, (send - end) as usize)))
		}
	}

	pub fn size (&self) -> usize
	{
		self.size
	}

	pub fn get_take_size (&self) -> Option<PageSize>
	{
		PageSize::try_from_usize (min (align_down_to_page_size (self.size), align_down_to_page_size (align_of (self.addr.as_u64 () as _))))
	}

	pub fn take (&mut self, size: PageSize) -> Option<PhysFrame>
	{
		let take_size = self.get_take_size ()?;
		if size > take_size
		{
			None
		}
		else
		{
			let size = size as usize;
			let addr = self.addr;
			self.addr += size;
			self.size -= size;
			Some(PhysFrame::new (addr, PageSize::from_usize (size)))
		}
	}

	pub fn iter (&self) -> PhysRangeIter
	{
		PhysRangeIter {
			start: self.addr,
			end: self.addr + self.size,
			life: PhantomData,
		}
	}
}

#[derive(Debug, Clone, Copy)]
pub struct PhysRangeIter<'a>
{
	start: PhysAddr,
	end: PhysAddr,
	life: PhantomData<&'a PhysRange>
}

// FIXME
impl Iterator for PhysRangeIter<'_>
{
	type Item = PhysFrame;

	fn next (&mut self) -> Option<Self::Item>
	{
		if self.start >= self.end
		{
			return None;
		}

		// wrong
		let size = min (align_of (self.start.as_u64 () as _),
			1 << log2 ((self.end - self.start) as _));
		let size = align_down_to_page_size (size);
		self.start += size;
		let size = PageSize::from_u64 (size as _);
		Some(PhysFrame::new (self.start, size))
	}
}

#[derive(Debug, Clone, Copy)]
pub enum VirtFrame
{
	K4(VirtAddr),
	M2(VirtAddr),
	G1(VirtAddr),
}

impl VirtFrame
{
	pub fn new (addr: VirtAddr, size: PageSize) -> Self
	{
		match size
		{
			PageSize::K4 => Self::K4(addr.align_down (size as u64)),
			PageSize::M2 => Self::M2(addr.align_down (size as u64)),
			PageSize::G1 => Self::G1(addr.align_down (size as u64)),
		}
	}

	pub fn start_addr (&self) -> VirtAddr
	{
		match self
		{
			Self::K4(addr) => *addr,
			Self::M2(addr) => *addr,
			Self::G1(addr) => *addr,
		}
	}

	pub fn end_addr (&self) -> VirtAddr
	{
		match self
		{
			Self::K4(addr) => *addr + PageSize::K4 as u64,
			Self::M2(addr) => *addr + PageSize::M2 as u64,
			Self::G1(addr) => *addr + PageSize::G1 as u64,
		}
	}

	pub fn get_size (&self) -> PageSize
	{
		match self
		{
			Self::K4(_) => PageSize::K4,
			Self::M2(_) => PageSize::M2,
			Self::G1(_) => PageSize::G1,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtRange
{
	// XXX: this field must be first because it is the first one compared
	addr: VirtAddr,
	size: usize,
}

impl VirtRange
{
	pub fn new (addr: VirtAddr, size: usize) -> Self
	{
		VirtRange {
			addr: addr.align_down (PageSize::K4 as u64),
			size: align_up (size, PageSize::K4 as _),
		}
	}

	pub fn new_unaligned (addr: VirtAddr, size: usize) -> Self
	{
		VirtRange {
			addr,
			size,
		}
	}

	pub fn null () -> Self
	{
		VirtRange {
			addr: VirtAddr::new (0),
			size: 0,
		}
	}

	pub fn aligned (&self) -> VirtRange
	{
		Self::new (self.addr, self.size)
	}

	pub fn addr (&self) -> VirtAddr
	{
		self.addr
	}

	pub fn as_usize (&self) -> usize
	{
		self.addr.as_u64 () as usize
	}

	pub fn end_addr (&self) -> VirtAddr
	{
		self.addr + self.size
	}

	pub fn end_usize (&self) -> usize
	{
		self.as_usize () + self.size
	}

	pub fn contains (&self, addr: VirtAddr) -> bool
	{
		(addr >= self.addr) && (addr < (self.addr + self.size))
	}

	pub fn contains_range (&self, range: Self) -> bool
	{
		self.contains (range.addr ()) || self.contains (range.addr () + range.size ())
	}

	pub fn split_at (&self, range: Self) -> (Option<VirtRange>, Option<VirtRange>)
	{
		let sbegin = self.addr;
		let send = self.addr + self.size;

		let begin = range.addr ();
		let end = begin + range.size ();

		if !self.contains_range (range)
		{
			(Some(*self), None)
		}
		else if begin <= sbegin && end >= send
		{
			(None, None)
		}
		else if self.contains (begin - 1u64) && !self.contains (end + 1u64)
		{
			(Some(VirtRange::new_unaligned (sbegin, (begin - sbegin) as usize)), None)
		}
		else if self.contains (end + 1u64) && !self.contains (begin - 1u64)
		{
			(Some(VirtRange::new_unaligned (end, (send - end) as usize)), None)
		}
		else
		{
			(Some(VirtRange::new_unaligned (sbegin, (begin - sbegin) as usize)),
				Some(VirtRange::new_unaligned (end, (send - end) as usize)))
		}
	}

	pub fn size (&self) -> usize
	{
		self.size
	}

	pub fn get_take_size (&self) -> Option<PageSize>
	{
		PageSize::try_from_usize (min (align_down_to_page_size (self.size), align_down_to_page_size (align_of (self.addr.as_u64 () as _))))
	}

	pub fn take (&mut self, size: PageSize) -> Option<VirtFrame>
	{
		let take_size = self.get_take_size ()?;
		if size > take_size
		{
			None
		}
		else
		{
			let size = size as usize;
			let addr = self.addr;
			self.addr += size;
			self.size -= size;
			Some(VirtFrame::new (addr, PageSize::from_usize (size)))
		}
	}

	pub fn iter (&self) -> VirtRangeIter
	{
		VirtRangeIter {
			start: self.addr,
			end: self.addr + self.size,
			life: PhantomData,
		}
	}
}

#[derive(Debug, Clone, Copy)]
pub struct VirtRangeIter<'a>
{
	start: VirtAddr,
	end: VirtAddr,
	life: PhantomData<&'a VirtRange>
}

// FIXME
impl Iterator for VirtRangeIter<'_>
{
	type Item = VirtFrame;

	fn next (&mut self) -> Option<Self::Item>
	{
		if self.start >= self.end
		{
			return None;
		}

		let size = min (align_of (self.start.as_u64 () as _),
			1 << log2 ((self.end - self.start) as _));
		let size = align_down_to_page_size (size);
		self.start += size;
		let size = PageSize::from_u64 (size as _);
		Some(VirtFrame::new (self.start, size))
	}
}

pub fn init (boot_info: &BootInfo)
{
	unsafe
	{
		phys_alloc::zm.init (boot_info);
	}
	kernel_heap::init ();
}
