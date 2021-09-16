use crate::uses::*;
use core::slice;
use crate::acpi::Rsdt;
use crate::mem::PhysRange;
use crate::consts;
use crate::util::{from_cstr, misc::phys_to_virt};

// multiboot tag type ids
const END: u32 = 0;
const MODULE: u32 = 3;
const MEMORY_MAP: u32 = 6;
const RSDP_OLD: u32 = 14;
const RSDP_NEW: u32 = 15;

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

impl TagHeader
{
	fn tag_ptr<T>(&self) -> Option<*const T>
	{
		if size_of::<T>() + size_of::<Self>() > self.size as usize {
			None
		}
		else
		{
			unsafe {
				Some((self as *const Self).add(1) as *const T)
			}
		}
	}

	unsafe fn tag_data<T>(&self) -> &T
	{
		self.tag_ptr::<T>().expect("tried to interpret data of mb2 tag as wrong type")
			.as_ref().unwrap()
	}
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

	fn deref(&self) -> &Self::Target
	{
		unsafe { core::slice::from_raw_parts(&self.data as *const _, self.len) }
	}
}

impl core::ops::DerefMut for MemoryMap
{
	fn deref_mut(&mut self) -> &mut Self::Target
	{
		unsafe { core::slice::from_raw_parts_mut(&mut self.data as *mut _, self.len) }
	}
}

impl MemoryMap
{
	fn new() -> Self
	{
		MemoryMap {
			data: [MemoryRegionType::None; MAX_MEMORY_REGIONS],
			len: 0,
		}
	}

	// pushes kernel zone on list if applicable
	fn push(&mut self, region: MemoryRegionType)
	{
		// this is kind of ugly to do here
		if region.range().addr()
			== consts::KERNEL_PHYS_RANGE.addr() + consts::KERNEL_PHYS_RANGE.size()
		{
			self.push(MemoryRegionType::Kernel(*consts::KERNEL_PHYS_RANGE));
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
	unsafe fn new_unchecked(region: &Mb2MemoryRegion) -> Self
	{
		let prange = PhysRange::new(PhysAddr::new(region.addr), region.len as usize);

		match region.typ {
			USABLE => Self::Usable(prange),
			ACPI => Self::Acpi(prange),
			HIBERNATE_PRESERVE => Self::HibernatePreserve(prange),
			DEFECTIVE => Self::Defective(prange),
			_ => Self::Reserved(prange),
		}
	}

	fn new(region: &Mb2MemoryRegion, initrd_range: PhysRange) -> [Option<Self>; 4]
	{
		let (prange1, prange2) =
			PhysRange::new_unaligned(PhysAddr::new(region.addr), region.len as usize)
				.split_at(*consts::KERNEL_PHYS_RANGE);

		let (prange1, prange3) = match prange1 {
			Some(prange) => prange.split_at(initrd_range),
			None => (None, None),
		};

		let (prange2, prange4) = match prange2 {
			Some(prange) => prange.split_at(initrd_range),
			None => (None, None),
		};

		let convert_func = |prange| match region.typ {
			USABLE => Self::Usable(prange),
			ACPI => Self::Acpi(prange),
			HIBERNATE_PRESERVE => Self::HibernatePreserve(prange),
			DEFECTIVE => Self::Defective(prange),
			_ => Self::Reserved(prange),
		};

		[prange1.map(convert_func), prange2.map(convert_func), prange3.map(convert_func), prange4.map(convert_func)]
	}

	fn range(&self) -> PhysRange
	{
		match self {
			Self::Usable(mem) => *mem,
			Self::Acpi(mem) => *mem,
			Self::HibernatePreserve(mem) => *mem,
			Self::Defective(mem) => *mem,
			Self::Reserved(mem) => *mem,
			Self::Kernel(mem) => *mem,
			Self::None => unreachable!(),
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

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct Mb2Module
{
	mod_start: u32,
	mod_end: u32,
}

impl Mb2Module
{
	unsafe fn string(&self) -> &str
	{
		let ptr = (self as *const Self).add(1) as *const u8;
		from_cstr(ptr).expect("bootloader did not pass valid utf-8 string for module name")
	}
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct Mb2RsdpOld {
	signature: [u8; 8],
	checksum: u8,
	oemid: [u8; 6],
	revision: u8,
	rsdt_addr: u32,
}

impl Mb2RsdpOld {
	// add up every byte and make sure lowest byte is equal to 0
	fn validate(&self) -> bool {
		let mut sum: usize = 0;
		let slice = unsafe {
			slice::from_raw_parts(self as *const _ as *const u8, size_of::<Self>())
		};

		for n in slice {
			sum += *n as usize;
		}

		sum % 0x100 == 0
	}
}

// multiboot 2 structure
#[derive(Debug, Clone, Copy)]
pub struct BootInfo<'a>
{
	pub memory_map: MemoryMap,
	pub initrd: &'a [u8],
	pub rsdt: &'a Rsdt,
}

impl BootInfo<'_>
{
	pub unsafe fn new(addr: usize) -> Self
	{
		// TODO: use an enum for each tag type, but since I only need memory map for now,
		// that would be a lot of extra typing

		// add 8 to get past initial entry which is always there
		let mut ptr = (addr + 8) as *const u8;

		let mut initrd_range = None;
		let mut initrd_slice = None;

		let mut memory_map = MemoryMap::new();
		let mut memory_map_tag = None;

		let mut rsdt = None;

		loop {
			let tag_header = (ptr as *const TagHeader).as_ref().unwrap();
			match tag_header.typ {
				END => break,
				MODULE => {
					let data: &Mb2Module = tag_header.tag_data();
					if data.string() == "initrd" {
						let size = (data.mod_end - data.mod_start) as usize;
						let paddr = PhysAddr::new(data.mod_start as u64);
						initrd_range = Some(PhysRange::new_unaligned(paddr, size));

						let initrd_ptr = phys_to_virt(paddr).as_u64() as *const u8;
						initrd_slice = Some(core::slice::from_raw_parts(initrd_ptr, size));
					}
				},
				MEMORY_MAP => memory_map_tag = Some(tag_header),
				RSDP_OLD => {
					let rsdp: &Mb2RsdpOld = tag_header.tag_data();
					if !rsdp.validate() {
						panic!("invalid rsdp passed to kernel");
					}
					rsdt = Rsdt::from(rsdp.rsdt_addr as usize);
				},
				RSDP_NEW => todo!(),
				_ => (),
			}

			ptr = ptr.add(align_up(tag_header.size as _, 8));
		}

		// have to do this at the end, because it needs to know where multiboot modules are
		if let Some(tag_header) = memory_map_tag {
			let mut ptr = (tag_header as *const _ as *const u8).add(16) as *const Mb2MemoryRegion;

			let len = (tag_header.size - 16) / 24;

			for _ in 0..len {
				let region = ptr.as_ref().unwrap();
	
				let regions = MemoryRegionType::new(region, initrd_range.expect("no initrd"));

				for region in regions {
					if let Some(region) = region {
						memory_map.push(region);
					}
				}

				ptr = ptr.add(1);
			}
		}

		BootInfo {
			memory_map,
			initrd: initrd_slice.expect("no initrd"),
			rsdt: rsdt.expect("no rsdt"),
		}
	}
}
