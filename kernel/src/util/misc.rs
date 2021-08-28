use crate::uses::*;
pub use libutil::misc::*;

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
	let a = vaddr.as_u64 ();
	if a < unsafe { MEM_OFFSET }
	{
		panic! ("virtual address was too small to convert to physical address");
	}
	// TODO: handle case when VirtAddr is bigger than meximum physical memory available
	PhysAddr::new (a - unsafe { MEM_OFFSET })
}

pub fn phys_to_virt_usize (paddr: usize) -> usize
{
	phys_to_virt (PhysAddr::new (paddr as u64)).as_u64 () as usize
}

pub fn virt_to_phys_usize (vaddr: usize) -> usize
{
	virt_to_phys (VirtAddr::new (vaddr as u64)).as_u64 () as usize
}

pub fn init (mem_offset: u64)
{
	unsafe
	{
		MEM_OFFSET = mem_offset;
	}
}
