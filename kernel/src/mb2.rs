use crate::uses::*;
use crate::mem::PhysRange;
use crate::consts;

// multiboot tag type ids
const END: u32 = 0;
const MEMORY_MAP: u32 = 6;

// multiboot memory type ids
// reserved is any other number
const USABLE: u32 = 1;
const ACPI: u32 = 3;
const HIBERNATE_PRESERVE: u32 = 4;
const DEFECTIVE: u32 = 5;

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct TagHeader
{
	typ: u32,
	size: u32,
}

const MAX_MEMORY_REGIONS: usize = 16;

#[derive(Debug, Clone, Copy)]
pub struct MemoryMap
{
	data: [MemoryRegionType; MAX_MEMORY_REGIONS],
	len: usize,
}

impl core::ops::Deref for MemoryMap
{
	type Target = [MemoryRegionType];

	fn deref (&self) -> &Self::Target
	{
		unsafe { core::slice::from_raw_parts (&self.data as *const _, self.len) }
	}
}

impl core::ops::DerefMut for MemoryMap
{
	fn deref_mut (&mut self) -> &mut Self::Target
	{
		unsafe { core::slice::from_raw_parts_mut (&mut self.data as *mut _, self.len) }
	}
}

impl MemoryMap
{
	fn new () -> Self
	{
		MemoryMap {
			data: [MemoryRegionType::None; MAX_MEMORY_REGIONS],
			len: 0,
		}
	}

	// pushes kernel zone on list if applicable
	fn push (&mut self, region: MemoryRegionType)
	{
		// this is kind of ugly to do here
		if region.range ().addr () == consts::KERNEL_PHYS_RANGE.addr () + consts::KERNEL_PHYS_RANGE.size ()
		{
			self.push (MemoryRegionType::Kernel(*consts::KERNEL_PHYS_RANGE));
		}
		assert!(self.len < MAX_MEMORY_REGIONS);
		self.data[self.len] = region;
		self.len += 1;
	}
}

#[derive(Debug, Clone, Copy)]
pub enum MemoryRegionType
{
	Usable(PhysRange),
	Acpi(PhysRange),
	HibernatePreserve(PhysRange),
	Defective(PhysRange),
	Reserved(PhysRange),
	Kernel(PhysRange),
	// only used internally, will never be shown if you deref a MemoryMap
	None,
}

impl MemoryRegionType
{
	// this one might overlap with the kernel
	unsafe fn new_unchecked (region: &Mb2MemoryRegion) -> Self
	{
		let prange = PhysRange::new (PhysAddr::new (region.addr), region.len as usize);

		match region.typ
		{
			USABLE => Self::Usable(prange),
			ACPI => Self::Acpi(prange),
			HIBERNATE_PRESERVE => Self::HibernatePreserve(prange),
			DEFECTIVE => Self::Defective(prange),
			_ => Self::Reserved(prange),
		}
	}

	fn new (region: &Mb2MemoryRegion) -> (Option<Self>, Option<Self>)
	{
		let (prange1, prange2) = PhysRange::new_unaligned (PhysAddr::new (region.addr), region.len as usize)
			.split_at (*consts::KERNEL_PHYS_RANGE);

		let convert_func = |prange| {
			match region.typ
			{
				USABLE => Self::Usable(prange),
				ACPI => Self::Acpi(prange),
				HIBERNATE_PRESERVE => Self::HibernatePreserve(prange),
				DEFECTIVE => Self::Defective(prange),
				_ => Self::Reserved(prange),
			}
		};

		(prange1.map (convert_func), prange2.map (convert_func))
	}

	fn range (&self) -> PhysRange
	{
		match self
		{
			Self::Usable(mem) => *mem,
			Self::Acpi(mem) => *mem,
			Self::HibernatePreserve(mem) => *mem,
			Self::Defective(mem) => *mem,
			Self::Reserved(mem) => *mem,
			Self::Kernel(mem) => *mem,
			Self::None => unreachable! (),
		}
	}
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct Mb2MemoryRegion
{
	addr: u64,
	len: u64,
	typ: u32,
	reserved: u32,
}

// multiboot 2 structure
#[derive(Debug, Clone, Copy)]
pub struct BootInfo
{
	pub memory_map: MemoryMap,
}

impl BootInfo
{
	pub unsafe fn new (addr: usize) -> Self
	{
		// TODO: use an enum for each tag type, but since I only need memory map for now,
		// that would be a lot of extra typing

		// add 8 to get past initial entry which is always there
		let mut ptr = (addr + 8) as *const u8;

		let mut memory_map = MemoryMap::new ();

		loop
		{
			let tag_header = (ptr as *const TagHeader).as_ref ().unwrap ();
			match tag_header.typ
			{
				END => break,
				MEMORY_MAP => {
					let mut rptr = ptr.add (16) as *const Mb2MemoryRegion;

					let len = (tag_header.size - 16) / 24;

					for _ in 0..len
					{
						let region = rptr.as_ref ().unwrap ();

						let (reg1, reg2) = MemoryRegionType::new (region);

						if let Some(reg1) = reg1
						{
							memory_map.push (reg1);
							if let Some(reg2) = reg2
							{
								memory_map.push (reg2);
							}
						}

						rptr = rptr.add (1);
					}

					ptr = ptr.add (align_up (tag_header.size as _, 8));
				},
				_ => ptr = ptr.add (align_up (tag_header.size as _, 8)),
			}
		}

		BootInfo {
			memory_map,
		}
	}
}
