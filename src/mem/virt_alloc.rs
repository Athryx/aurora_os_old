use spin::Mutex;
use bitflags::bitflags;
use alloc::collections::BTreeMap;
use crate::uses::*;
use crate::arch::x64::{invlpg, get_cr3, set_cr3};
use crate::consts;
use super::phys_alloc::{Allocation, ZoneManager, zm};
use super::*;

const PAGE_ADDR_BITMASK: usize = 0x000ffffffffff000;
lazy_static!
{
	static ref MAX_MAP_ADDR: usize = consts::KERNEL_VIRT_RANGE.as_usize ();

	// TODO: make global
	static ref HIGHER_HALF_PAGE_POINTER: PageTablePointer = PageTablePointer::new (*consts::KZONE_PAGE_TABLE_POINTER,
		PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::SUPERUSER);
}

pub type FAllocerType = ZoneManager;

pub unsafe trait FrameAllocator
{
	// implementor must guarentee that constructing a new allocation with same address and size of 1 page will work to free
	fn alloc_frame (&self) -> Allocation;
	unsafe fn dealloc_frame (&self, frame: Allocation);
}

bitflags!
{
	pub struct PageTableFlags: usize
	{
		const NONE = 		0;
		const PRESENT = 	1;
		const WRITABLE = 	1 << 1;
		const SUPERUSER = 	1 << 2;
		const PWT = 		1 << 3;
		const PCD = 		1 << 4;
		const ACCESSED = 	1 << 5;
		const DIRTY = 		1 << 6;
		const HUGE = 		1 << 7;
		const GLOBAL = 		1 << 8;
		const NO_EXEC =		1 << 63;
	}
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
struct PageTablePointer(usize);

impl PageTablePointer
{
	fn new (addr: PhysAddr, flags: PageTableFlags) -> Self
	{
		let addr = addr.as_u64 () as usize;
		PageTablePointer(addr | flags.bits ())
	}

	unsafe fn as_ref<'a, 'b> (&'a self) -> Option<&'b PageTable>
	{
		if self.0 & PageTableFlags::PRESENT.bits () == 0
		{
			None
		}
		else
		{
			let addr = phys_to_virt (PhysAddr::new ((self.0 & PAGE_ADDR_BITMASK) as u64));
			Some((addr.as_u64 () as *const PageTable).as_ref ().unwrap ())
		}
	}

	unsafe fn as_mut<'a, 'b> (&'a mut self) -> Option<&'b mut PageTable>
	{
		if self.0 & PageTableFlags::PRESENT.bits () == 0
		{
			None
		}
		else
		{
			let addr = phys_to_virt (PhysAddr::new ((self.0 & PAGE_ADDR_BITMASK) as u64));
			Some((addr.as_u64 () as *mut PageTable).as_mut ().unwrap ())
		}
	}

	unsafe fn set_flags (&mut self, flags: PageTableFlags)
	{
		self.0 = (self.0 & PAGE_ADDR_BITMASK) | flags.bits ();
	}
}

#[repr(transparent)]
#[derive(Debug)]
struct PageTable([PageTablePointer; PAGE_SIZE / 8]);

impl PageTable
{
	fn new<T: FrameAllocator> (allocer: &T, flags: PageTableFlags, dropable: bool) -> PageTablePointer
	{
		let frame = allocer.alloc_frame ().as_usize ();
		let addr = virt_to_phys_usize (frame);
		let flags = flags | PageTableFlags::PRESENT;
		let mut out = PageTablePointer(addr | flags.bits ());
		if !dropable
		{
			unsafe { out.as_mut ().unwrap ().set_count (1); }
		}
		out
	}

	fn count (&self) -> usize
	{
		get_bits (self.0[0].0, 52..63)
	}

	fn set_count (&mut self, n: usize)
	{
		let n = get_bits (n, 0..11);
		let ptr_no_count = self.0[0].0 & 0x800fffffffffffff;
		self.0[0] = PageTablePointer(ptr_no_count | (n << 52));
	}

	fn inc_count (&mut self, n: isize)
	{
		self.set_count ((self.count () as isize - n) as _);
	}

	fn present (&self, index: usize) -> bool
	{
		(self.0[index].0 & PageTableFlags::PRESENT.bits ()) != 0
	}

	// TODO: make this more safe
	unsafe fn free_if_empty<'a, T: FrameAllocator + 'a> (&mut self, allocer: &'a T) -> bool
	{
		if self.count () == 0
		{
			let frame = Allocation::new (self.addr (), PAGE_SIZE);
			allocer.dealloc_frame (frame);
			true
		}
		else
		{
			false
		}
	}

	fn set (&mut self, index: usize, ptr: PageTablePointer)
	{
		assert! (!self.present (index));
		self.0[index] = ptr;
		self.inc_count (1);
	}
	
	fn get<'a, 'b> (&'a mut self, index: usize) -> &'b mut PageTable
	{
		unsafe { self.0[index].as_mut ().unwrap () }
	}

	fn get_or_alloc<'a, 'b, T: FrameAllocator + 'a> (&'a mut self, index: usize, allocer: &'b T, flags: PageTableFlags) -> &'a mut PageTable
	{
		if self.present (index)
		{
			unsafe { self.0[index].as_mut ().unwrap () }
		}
		else
		{
			let mut out = PageTable::new (allocer, flags, true);
			self.set (index, out);
			unsafe { out.as_mut ().unwrap () }
		}
	}

	// returns true if dropped
	unsafe fn remove<T: FrameAllocator> (&mut self, index: usize, allocer: &T) -> bool
	{
		let n = self.0[index].0;
		if !self.present (index)
		{
			self.0[index] = PageTablePointer(n & !PageTableFlags::PRESENT.bits ());
			self.inc_count (-1);
			self.free_if_empty (allocer)
		}
		else
		{
			false
		}
	}

	fn addr (&self) -> usize
	{
		self as *const _ as usize
	}
}

#[derive(Debug, Clone, Copy)]
pub enum VirtLayoutElement
{
	Mem(PhysRange),
	// will translate this to physical address
	AllocedMem(Allocation),
	Empty(usize),
}

impl VirtLayoutElement
{
	pub fn size (&self) -> usize
	{
		match self
		{
			Self::Mem(mem) => mem.size (),
			Self::AllocedMem(mem) => mem.len (),
			Self::Empty(size) => *size,
		}
	}
}

// TODO: ensure all sizes are page aligned
#[derive(Debug, Clone)]
pub struct VirtLayout(Vec<VirtLayoutElement>);

impl VirtLayout
{
	// vec must have length greater than 0
	pub fn new (vec: Vec<VirtLayoutElement>) -> Self
	{
		VirtLayout(vec)
	}

	// vec must have length greater than 0, otherwise None is returned
	pub fn try_new (vec: Vec<VirtLayoutElement>) -> Option<Self>
	{
		if vec.len () > 0
		{
			Some(VirtLayout(vec))
		}
		else
		{
			None
		}
	}

	pub fn size (&self) -> usize
	{
		self.0.iter ().fold (0, |n, a| n + a.size ())
	}

	// must onlt be called once
	pub unsafe fn dealloc (&self)
	{
		for a in self.0.iter ()
		{
			match a
			{
				VirtLayoutElement::AllocedMem(allocation) => zm.dealloc (*allocation),
				_ => (),
			}
		}
	}
}

#[must_use = "unused Unflushed, call flush to flush tlb cache"]
#[derive(Debug)]
pub struct Unflushed<'a, T: FrameAllocator + 'static>
{
	virt_zone: VirtRange,
	mapper: &'a VirtMapper<T>,
}

impl<T: FrameAllocator> Unflushed<'_, T>
{
	fn new (virt_zone: VirtRange, mapper: &VirtMapper<T>) -> Unflushed<'_, T>
	{
		Unflushed {
			virt_zone,
			mapper,
		}
	}

	pub fn flush (&self) -> VirtRange
	{
		//self.mapper.flush (self.virt_zone);
		self.virt_zone
	}
}

#[derive(Debug)]
struct PageMappingIterator
{
	virt_zone: VirtRange,
	phys_zone: VirtLayout,
	pindex: usize,
}

impl PageMappingIterator
{
	fn new (phys_zone: VirtLayout, virt_zone: VirtRange) -> Self
	{
		PageMappingIterator {
			virt_zone,
			phys_zone,
			pindex: 0,
		}
	}
}

impl Iterator for PageMappingIterator
{
	type Item = (PhysFrame, VirtFrame);

	fn next (&mut self) -> Option<Self::Item>
	{
		if self.pindex == self.phys_zone.0.len () || self.virt_zone.size () < PAGE_SIZE
		{
			return None;
		}

		let vsize = self.virt_zone.get_take_size ();
		let psize = match self.phys_zone.0[self.pindex]
		{
			VirtLayoutElement::AllocedMem(mem) => {
				let prange = mem.as_phys_zone ();
				self.phys_zone.0[self.pindex] = VirtLayoutElement::Mem(prange);
				prange.get_take_size ()
			}
			VirtLayoutElement::Mem(mem) => mem.get_take_size (),
			VirtLayoutElement::Empty(mem) => PageSize::from_usize (align_down_to_page_size (mem)),
		};

		let size = min (vsize, psize);

		let vframe = self.virt_zone.take (size).unwrap ();
		let pframe = match self.phys_zone.0[self.pindex]
		{
			VirtLayoutElement::Mem(mut mem) => {
				let out = mem.take (size).unwrap ();
				if mem.size () < PAGE_SIZE
				{
					self.pindex += 1;
				}
				out
			},
			VirtLayoutElement::Empty(ref mut mem) => {
				*mem -= size as usize;
				if *mem < PAGE_SIZE
				{
					self.pindex += 1;
				}
				PhysFrame::new (PhysAddr::new (0), size)
			},
			VirtLayoutElement::AllocedMem(_) => unreachable! (),
		};

		Some((pframe, vframe))
	}
}

#[derive(Debug)]
pub struct VirtMapper<T: FrameAllocator + 'static>
{
	virt_map: Mutex<BTreeMap<VirtRange, VirtLayout>>,
	cr3: Mutex<PageTablePointer>,
	frame_allocer: &'static T,
}

impl<T: FrameAllocator> VirtMapper<T>
{
	// TODO: lazy tlb flushing
	pub fn new (frame_allocer: &'static T) -> VirtMapper<T>
	{
		let mut pml4_table = PageTable::new (frame_allocer, PageTableFlags::PRESENT, false);
		// NOTE: change index if kernel_vma changes
		unsafe
		{
			pml4_table.as_mut ().unwrap ().set (511, *HIGHER_HALF_PAGE_POINTER);
		}
		VirtMapper {
			virt_map: Mutex::new (BTreeMap::new ()),
			cr3: Mutex::new (pml4_table),
			frame_allocer,
		}
	}

	pub fn set_frame_allocator (&mut self, frame_allocer: &'static T)
	{
		self.frame_allocer = frame_allocer;
	}

	pub unsafe fn load (&self)
	{
		set_cr3 (self.cr3.lock ().0);
	}

	pub fn is_loaded (&self) -> bool
	{
		self.cr3.lock ().0 == get_cr3 ()
	}

	fn flush (&self, _virt_zone: VirtRange)
	{
		unimplemented! ();
	}

	pub unsafe fn map (&self, phys_zones: VirtLayout, flags: PageTableFlags) -> Result<VirtRange, Err>
	{
		// TODO: choose better zones based off alignment so more big pages cna be used saving tlb cache space
		let size = phys_zones.size ();
		let mut laddr = 0;
		let mut found = false;

		let mut btree = self.virt_map.lock ();

		for zone in btree.keys ()
		{
			let diff = zone.as_usize () - laddr;
			if diff >= size
			{
				found = true;
				break;
			}
			laddr = zone.as_usize () + zone.size ();
		}

		if !found && (*MAX_MAP_ADDR - laddr < size)
		{
			return Err(Err::new ("not enough space in virtual memory space for allocation"));
		}

		let virt_zone = VirtRange::new (VirtAddr::new (laddr as _), size);

		btree.insert (virt_zone, phys_zones.clone ());

		self.map_at_unchecked (phys_zones, virt_zone, flags)
	}

	pub unsafe fn map_at (&self, phys_zones: VirtLayout, virt_zone: VirtRange, flags: PageTableFlags) -> Result<VirtRange, Err>
	{
		if phys_zones.size () != virt_zone.size ()
		{
			return Err(Err::new ("phys_zones and virt_zone size did not match"));
		}

		if virt_zone.end_usize () >= *MAX_MAP_ADDR
		{
			return Err(Err::new ("attempted to map an address in the higher half kernel zone"));
		}

		let mut btree = self.virt_map.lock ();

		let prev = btree.range (..virt_zone).next_back ();
		let next = btree.range (virt_zone..).next ();

		if let Some((prev, _)) = prev
		{
			if prev.addr () + prev.size () > virt_zone.addr ()
			{
				return Err(Err::new ("invalid virt zone passed to map_at"));
			}
		}

		if let Some((next, _)) = next
		{
			if virt_zone.addr () + virt_zone.size () > next.addr ()
			{
				return Err(Err::new ("invalid virt zone passed to map_at"));
			}
		}

		btree.insert (virt_zone, phys_zones.clone ());

		self.map_at_unchecked (phys_zones, virt_zone, flags)
	}

	// TODO: improve performance by caching previous virt parents
	unsafe fn map_at_unchecked (&self, phys_zones: VirtLayout, virt_zone: VirtRange, flags: PageTableFlags) -> Result<VirtRange, Err>
	{
		let iter = PageMappingIterator::new (phys_zones, virt_zone);
		for (pframe, vframe) in iter
		{
			let addr = vframe.start_addr ().as_u64 () as usize;
			let nums = [
				get_bits (addr, 39..48),
				get_bits (addr, 30..39),
				get_bits (addr, 21..30),
				get_bits (addr, 12..21),
			];

			let pf = if pframe.start_addr () == PhysAddr::new (0)
				{ PageTableFlags::NONE } else { PageTableFlags::PRESENT };

			let (depth, hf) = match pframe
			{
				PhysFrame::K4(_) => (4, PageTableFlags::NONE),
				PhysFrame::M2(_) => (3, PageTableFlags::HUGE),
				PhysFrame::G1(_) => (2, PageTableFlags::HUGE),
			};

			let mut ptable = self.cr3.lock ().as_mut ().unwrap ();

			for a in 0..depth
			{
				let i = nums[a];
				if a == depth - 1
				{
					let flags = flags | pf | hf;
					ptable.set (i, PageTablePointer::new (pframe.start_addr (), flags));
				}
				else
				{
					ptable = ptable.get_or_alloc (i, self.frame_allocer, flags);
				}
			}

			// TODO: check if address space is loaded before updating tlb cache
			invlpg (addr);
		}

		Ok(virt_zone)
	}

	pub unsafe fn unmap (&self, virt_zone: VirtRange) -> Result<VirtLayout, Err>
	{
		let phys_zones = self.virt_map.lock ().remove (&virt_zone)?;

		let iter = PageMappingIterator::new (phys_zones.clone (), virt_zone);
		for (pframe, vframe) in iter
		{
			let addr = vframe.start_addr ().as_u64 () as usize;
			let nums = [
				get_bits (addr, 39..48),
				get_bits (addr, 30..39),
				get_bits (addr, 21..30),
				get_bits (addr, 12..21),
			];

			let depth = match pframe
			{
				PhysFrame::K4(_) => 4,
				PhysFrame::M2(_) => 3,
				PhysFrame::G1(_) => 2,
			};

			let mut tables = [Some(self.cr3.lock ().as_mut ().unwrap ()), None, None, None];

			for a in 1..depth
			{
				tables[a] = Some(tables[a - 1].as_mut ().unwrap ().get (nums[a - 1]));
			}

			for a in (depth - 1)..=0
			{
				if !tables[a].as_mut ().unwrap ().remove (nums[a + 1], self.frame_allocer)
				{
					break;
				}
			}

			// TODO: check if address space is loaded before updating tlb cache
			invlpg (addr);
		}

		Ok(phys_zones)
	}
}
