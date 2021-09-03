use core::cmp::min;
use core::marker::PhantomData;
use core::slice;

use phys_alloc::Allocation;

use crate::uses::*;
use crate::syscall::udata;

// unless otherwise stated, all lens in this module are in bytes, not pages
// TODO: make traits or macros to reduce duplicated code on Phys and Virt versions of all these types

pub mod error;
pub mod phys_alloc;
pub mod shared_mem;
pub mod sys;
pub mod virt_alloc;

pub const PAGE_SIZE: usize = 4096;
pub const MAX_VIRT_ADDR: usize = 1 << 47;

pub fn align_down_to_page_size(n: usize) -> usize
{
	if n >= PageSize::G1 as usize {
		PageSize::G1 as usize
	} else if n >= PageSize::M2 as usize {
		PageSize::M2 as usize
	} else if n >= PageSize::K4 as usize {
		PageSize::K4 as usize
	} else {
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
	pub fn from_u64(n: u64) -> Self
	{
		Self::from_usize(n as _)
	}

	pub fn from_usize(n: usize) -> Self
	{
		Self::try_from_usize(n)
			.expect("tried to convert integer to PageSize, but it wasn't a valid page size")
	}

	pub fn try_from_usize(n: usize) -> Option<Self>
	{
		match n {
			0x1000 => Some(Self::K4),
			0x200000 => Some(Self::M2),
			0x40000000 => Some(Self::G1),
			_ => None,
		}
	}
}

macro_rules! impl_addr_range {
	($addr:ident, $frame:ident, $range:ident, $iter:ident) => {
		#[derive(Debug, Clone, Copy)]
		pub enum $frame
		{
			K4($addr),
			M2($addr),
			G1($addr),
		}

		impl $frame
		{
			pub fn new(addr: $addr, size: PageSize) -> Self
			{
				match size {
					PageSize::K4 => Self::K4(addr.align_down(size as u64)),
					PageSize::M2 => Self::M2(addr.align_down(size as u64)),
					PageSize::G1 => Self::G1(addr.align_down(size as u64)),
				}
			}

			pub fn start_addr(&self) -> $addr
			{
				match self {
					Self::K4(addr) => *addr,
					Self::M2(addr) => *addr,
					Self::G1(addr) => *addr,
				}
			}

			pub fn end_addr(&self) -> $addr
			{
				match self {
					Self::K4(addr) => *addr + PageSize::K4 as u64,
					Self::M2(addr) => *addr + PageSize::M2 as u64,
					Self::G1(addr) => *addr + PageSize::G1 as u64,
				}
			}

			pub fn get_size(&self) -> PageSize
			{
				match self {
					Self::K4(_) => PageSize::K4,
					Self::M2(_) => PageSize::M2,
					Self::G1(_) => PageSize::G1,
				}
			}
		}

		#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
		pub struct $range
		{
			// NOTE: this field must be first because it is the first one compared
			addr: $addr,
			size: usize,
		}

		impl $range
		{
			pub fn new(addr: $addr, size: usize) -> Self
			{
				Self {
					addr: addr.align_down(PageSize::K4 as u64),
					size: align_up(size, PageSize::K4 as _),
				}
			}

			// returns error if invalid virt addr or unaligned addr and size
			pub fn try_new_usize(addr: usize, size: usize) -> Result<Self, SysErr>
			{
				let vaddr = $addr::try_new(addr as u64).or(Err(SysErr::InvlVirtAddr))?;
				if align_of(addr) < PAGE_SIZE || align_of(size) < PAGE_SIZE {
					Err(SysErr::InvlPtr)
				} else {
					Ok(Self {
						addr: vaddr,
						size,
					})
				}
			}

			// returns error if invalid virt addr or unaligned addr and size, or not in user mem zone
			pub fn try_new_user(addr: usize, size: usize) -> Result<Self, SysErr>
			{
				let out = Self::try_new_usize(addr, size)?;
				if !out.verify_umem() {
					Err(SysErr::InvlPtr)
				} else {
					Ok(out)
				}
			}

			// this is a bugfix for new, but I am making a new fucntion incase anything relied on the old behavior of new
			pub fn new_aligned(addr: $addr, size: usize) -> Self
			{
				let addr2 = (addr + size).align_up(PageSize::K4 as u64);
				Self {
					addr: addr.align_down(PageSize::K4 as u64),
					size: align_up((addr2 - addr) as usize, PageSize::K4 as usize),
				}
			}

			pub fn new_unaligned(addr: $addr, size: usize) -> Self
			{
				Self {
					addr,
					size,
				}
			}

			pub fn null() -> Self
			{
				Self {
					addr: $addr::new(0),
					size: 0,
				}
			}

			pub fn aligned(&self) -> Self
			{
				Self::new_aligned(self.addr, self.size)
			}

			pub fn is_aligned(&self) -> bool
			{
				align_of(self.as_usize()) >= PAGE_SIZE && align_of(self.end_usize()) >= PAGE_SIZE
			}

			pub fn addr(&self) -> $addr
			{
				self.addr
			}

			pub fn as_usize(&self) -> usize
			{
				self.addr.as_u64() as usize
			}

			pub fn end_addr(&self) -> $addr
			{
				self.addr + self.size
			}

			pub fn end_usize(&self) -> usize
			{
				self.as_usize() + self.size
			}

			pub unsafe fn as_slice(&self) -> &[u8]
			{
				slice::from_raw_parts(self.as_usize() as *const u8, self.size)
			}

			pub fn contains(&self, addr: $addr) -> bool
			{
				(addr >= self.addr) && (addr < (self.addr + self.size))
			}

			// NOTE: this only returns if it intersects at all, not if the range is fully contained in this range
			pub fn contains_range(&self, range: Self) -> bool
			{
				self.contains(range.addr()) || self.contains(range.addr() + range.size())
			}

			pub fn full_contains_range(&self, range: Self) -> bool
			{
				self.contains(range.addr()) && self.contains(range.addr() + range.size())
			}

			pub fn verify_umem(&self) -> bool
			{
				udata::verify_umem(self.as_usize(), self.size)
			}

			pub fn merge(&self, other: Self) -> Option<Self>
			{
				if self.end_addr() == other.addr() {
					Some(Self::new_unaligned(self.addr(), self.size() + other.size()))
				} else if other.end_addr() == self.addr() {
					Some(Self::new_unaligned(
						other.addr(),
						self.size() + other.size(),
					))
				} else {
					None
				}
			}

			pub fn split_at(&self, range: Self) -> (Option<Self>, Option<Self>)
			{
				let sbegin = self.addr;
				let send = self.addr + self.size;

				let begin = range.addr();
				let end = begin + range.size();

				if !self.contains_range(range) {
					(Some(*self), None)
				} else if begin <= sbegin && end >= send {
					(None, None)
				} else if self.contains(begin - 1u64) && !self.contains(end + 1u64) {
					(
						Some(Self::new_unaligned(sbegin, (begin - sbegin) as usize)),
						None,
					)
				} else if self.contains(end + 1u64) && !self.contains(begin - 1u64) {
					(Some(Self::new_unaligned(end, (send - end) as usize)), None)
				} else {
					(
						Some(Self::new_unaligned(sbegin, (begin - sbegin) as usize)),
						Some(Self::new_unaligned(end, (send - end) as usize)),
					)
				}
			}

			pub fn size(&self) -> usize
			{
				self.size
			}

			pub fn get_take_size(&self) -> Option<PageSize>
			{
				PageSize::try_from_usize(min(
					align_down_to_page_size(self.size),
					align_down_to_page_size(align_of(self.addr.as_u64() as _)),
				))
			}

			pub fn take(&mut self, size: PageSize) -> Option<$frame>
			{
				let take_size = self.get_take_size()?;
				if size > take_size {
					None
				} else {
					let size = size as usize;
					let addr = self.addr;
					self.addr += size;
					self.size -= size;
					Some($frame::new(addr, PageSize::from_usize(size)))
				}
			}

			pub fn iter(&self) -> $iter
			{
				$iter {
					start: self.addr,
					end: self.addr + self.size,
					life: PhantomData,
				}
			}
		}

		#[derive(Debug, Clone, Copy)]
		pub struct $iter<'a>
		{
			start: $addr,
			end: $addr,
			life: PhantomData<&'a $range>,
		}

		// FIXME
		impl Iterator for $iter<'_>
		{
			type Item = $frame;

			fn next(&mut self) -> Option<Self::Item>
			{
				if self.start >= self.end {
					return None;
				}

				let size = min(
					align_of(self.start.as_u64() as _),
					1 << log2((self.end - self.start) as _),
				);
				let size = align_down_to_page_size(size);
				self.start += size;
				let size = PageSize::from_u64(size as _);
				Some($frame::new(self.start, size))
			}
		}
	};
}

impl_addr_range! {PhysAddr, PhysFrame, PhysRange, PhysRangeIter}
impl_addr_range! {VirtAddr, VirtFrame, VirtRange, VirtRangeIter}

impl From<Allocation> for PhysRange
{
	fn from(mem: Allocation) -> Self
	{
		Self::new(virt_to_phys(mem.addr()), mem.len())
	}
}
