use crate::uses::*;
use core::slice;
use crate::mem::{PhysRange, VirtRange};

extern "C"
{
	// virtual address that physical memory is offset by (includes 1 extra megabyte) (does include lower half of kernel)
	static __KERNEL_VMA: usize;
	// physical address kernel resides at (does not include 1 extra megabyte) (does include lower half of kernel)
	static __KERNEL_LMA: usize;
	static __TEXT_START: usize;
	static __TEXT_END: usize;
	static __RODATA_START: usize;
	static __RODATA_END: usize;
	static __DATA_START: usize;
	static __DATA_END: usize;
	static __BSS_START: usize;
	static __BSS_END: usize;
	// virtual address that kernal starts at (does not include 1 extra megabyte) (does include lower half of kernel)
	static __KERNEL_START: usize;
	// virtual address that kernel ends at
	static __KERNEL_END: usize;
	static stack_bottom: usize;
	static stack_top: usize;
	static PDP_table: usize;

	static initfs: u8;
	static initfs_len: usize;
}

lazy_static!
{
	pub static ref KERNEL_VMA: usize = unsafe { &__KERNEL_VMA } as *const _ as usize;
	pub static ref KERNEL_LMA: usize = unsafe { &__KERNEL_LMA } as *const _ as usize;
	pub static ref TEXT_START: usize = unsafe { &__TEXT_START } as *const _ as usize;
	pub static ref TEXT_END: usize = unsafe { &__TEXT_END } as *const _ as usize;
	pub static ref RODATA_START: usize = unsafe { &__RODATA_START } as *const _ as usize;
	pub static ref RODATA_END: usize = unsafe { &__RODATA_END } as *const _ as usize;
	pub static ref DATA_START: usize = unsafe { &__DATA_START } as *const _ as usize;
	pub static ref DATA_END: usize = unsafe { &__DATA_END } as *const _ as usize;
	pub static ref BSS_START: usize = unsafe { &__BSS_START } as *const _ as usize;
	pub static ref BSS_END: usize = unsafe { &__BSS_END } as *const _ as usize;
	pub static ref KERNEL_START: usize = unsafe { &__KERNEL_START } as *const _ as usize;
	pub static ref KERNEL_END: usize = unsafe { &__KERNEL_END } as *const _ as usize;

	pub static ref KERNEL_PHYS_RANGE: PhysRange = PhysRange::new (PhysAddr::new (*KERNEL_LMA as u64), *KERNEL_END - *KERNEL_START);
	pub static ref KERNEL_VIRT_RANGE: VirtRange = VirtRange::new (VirtAddr::new (*KERNEL_START as u64), *KERNEL_END - *KERNEL_START);

	pub static ref INIT_STACK: VirtRange = VirtRange::new (phys_to_virt (PhysAddr::new (unsafe { &stack_bottom } as *const _ as u64)),
		(unsafe { &stack_top } as *const _ as usize) - (unsafe { &stack_bottom } as *const _ as usize));

	pub static ref KZONE_PAGE_TABLE_POINTER: PhysAddr = PhysAddr::new (unsafe { &PDP_table } as *const _ as u64);

	pub static ref INITFS: &'static [u8] = unsafe { slice::from_raw_parts (&initfs as *const u8, &initfs_len as *const _ as usize) };
}
