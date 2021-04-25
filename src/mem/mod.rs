use core::cmp::min;
use core::marker::PhantomData;
use crate::uses::*;
use bootloader::bootinfo::BootInfo;

// unless otherwise stated, all lens in this module are in bytes, not pages

pub mod phys_alloc;
pub mod virt_alloc;
pub mod kernel_heap;

pub const PAGE_SIZE: usize = 4096;
pub const MAX_VIRT_ADDR: usize = 1 << 47;

pub fn align_down_to_page_size (n: usize) -> usize
{
	if n > PageSize::G1 as usize
	{
		PageSize::G1 as usize
	}
	else if n > PageSize::M2 as usize
	{
		PageSize::M2 as usize
	}
	else if n > PageSize::K4 as usize
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
		match n
		{
			0x1000 => Self::K4,
			0x200000 => Self::M2,
			0x40000000 => Self::G1,
			_ => panic! ("tried to convert u64 to PageSize, but it wasn't a valid page size"),
		}
	}

	pub fn from_usize (n: usize) -> Self
	{
		match n
		{
			0x1000 => Self::K4,
			0x200000 => Self::M2,
			0x40000000 => Self::G1,
			_ => panic! ("tried to convert usize to PageSize, but it wasn't a valid page size"),
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
			size: align_down (size, PageSize::K4 as _),
		}
	}

	pub fn addr (&self) -> PhysAddr
	{
		self.addr
	}

	pub fn as_usize (&self) -> usize
	{
		self.addr.as_u64 () as usize
	}

	pub fn size (&self) -> usize
	{
		self.size
	}

	pub fn get_take_size (&self) -> PageSize
	{
		PageSize::from_usize (min (align_down_to_page_size (self.size), align_of (self.addr.as_u64 () as _)))
	}

	pub fn take (&mut self, size: PageSize) -> Option<PhysFrame>
	{
		if size > self.get_take_size ()
		{
			None
		}
		else
		{
			let size = size as usize;
			self.addr += size;
			self.size -= size;
			Some(PhysFrame::new (self.addr, PageSize::from_usize (size)))
		}
	}

	pub fn iter<'a> (&'a self) -> PhysRangeIter<'a>
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
			size: align_down (size, PageSize::K4 as _),
		}
	}

	pub fn addr (&self) -> VirtAddr
	{
		self.addr
	}

	pub fn as_usize (&self) -> usize
	{
		self.addr.as_u64 () as usize
	}

	pub fn size (&self) -> usize
	{
		self.size
	}

	pub fn get_take_size (&self) -> PageSize
	{
		PageSize::from_usize (min (align_down_to_page_size (self.size), align_of (self.addr.as_u64 () as _)))
	}

	pub fn take (&mut self, size: PageSize) -> Option<VirtFrame>
	{
		if size > self.get_take_size ()
		{
			None
		}
		else
		{
			let size = size as usize;
			self.addr += size;
			self.size -= size;
			Some(VirtFrame::new (self.addr, PageSize::from_usize (size)))
		}
	}

	pub fn iter<'a> (&'a self) -> VirtRangeIter<'a>
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

pub fn init (boot_info: &'static BootInfo)
{
	unsafe
	{
		phys_alloc::zm.init (boot_info);
	}
	kernel_heap::init ();
}
