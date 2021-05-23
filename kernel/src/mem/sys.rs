use crate::uses::*;
use crate::syscall::{SyscallVals, consts};
use super::{PAGE_SIZE, VirtRange};
use super::phys_alloc::zm;
use super::virt_alloc::{VirtLayoutElement, VirtLayout, PageMappingFlags};
use crate::sched::proc_c;

const READ: u32 = 1;
const WRITE: u32 = 1 << 1;
const EXEC: u32 = 1 << 2;

const REALLOC_EXACT: usize = 1 << 4;

#[repr(usize)]
enum Realloc
{
	Ok = 0,
	OutOfMem = 1,
	AllocAtInvlAddr = 2,
	InvlPointer = 3,
	Unknown = 4,
}

// FIXME: this doesn't return the right error codes yet
// FIXME: this doesn't obey REALLOC_EXACT
// FIXME: this doesn't support resizing yet
pub extern "C" fn realloc (vals: &mut SyscallVals)
{
	let options = vals.options;
	let addr = align_down (vals.a1, PAGE_SIZE);
	let size = vals.a2 * PAGE_SIZE;
	let at_addr = align_down (vals.a3, PAGE_SIZE);

	let flags = PageMappingFlags::from_bits_truncate (options as usize)
		| PageMappingFlags::USER;

	if addr == 0
	{
		// allocate memory
		if size == 0
		{
			// no need to set values
			// they are already 0
			// but it is easier to read
			vals.a1 = 0;
			vals.a2 = 0;
			vals.a3 = Realloc::Ok as usize;
			return;
		}

		let layout_element = match VirtLayoutElement::new (size, flags)
		{
			Some(elem) => elem,
			None => {
				vals.a1 = 0;
				vals.a2 = 0;
				vals.a3 = Realloc::OutOfMem as usize;
				return;
			}
		};

		let vec = vec![layout_element];

		let layout = VirtLayout::from (vec);

		if at_addr == 0
		{
			unsafe
			{
				match proc_c ().addr_space.map (layout)
				{
					Ok(virt_range) => {
						vals.a1 = virt_range.as_usize ();
						vals.a2 = virt_range.size () / PAGE_SIZE;
						vals.a3 = 0;
					},
					Err(_) => {
						vals.a1 = 0;
						vals.a2 = 0;
						vals.a3 = Realloc::OutOfMem as usize;
					},
				}
				return;
			}
		}
		else
		{
			let virt_zone = VirtRange::new (VirtAddr::new_truncate (at_addr as u64), layout.size ());
			unsafe
			{
				match proc_c ().addr_space.map_at (layout, virt_zone)
				{
					Ok(virt_range) => {
						vals.a1 = virt_range.as_usize ();
						vals.a2 = virt_range.size () / PAGE_SIZE;
						vals.a3 = 0;
					},
					Err(_) => {
						vals.a1 = 0;
						vals.a2 = 0;
						vals.a3 = Realloc::OutOfMem as usize;
					},
				}
			}
		}
	}
	else if size == 0
	{
		// free memory
		let mapper = &proc_c ().addr_space;

		let virt_zone = match mapper.get_mapped_range (VirtAddr::new_truncate (addr as u64))
		{
			Some(range) => range,
			None => {
				vals.a1 = 0;
				vals.a2 = 0;
				vals.a3 = Realloc::InvlPointer as usize;
				return;
			}
		};

		match unsafe { mapper.unmap (virt_zone) }
		{
			Ok(layout) => {
				unsafe { layout.dealloc () };
				vals.a1 = 0;
				vals.a2 = 0;
				vals.a3 = 0;
			},
			Err(_) => {
				vals.a1 = 0;
				vals.a2 = 0;
				vals.a3 = Realloc::Unknown as usize;
			}
		}
	}
	else
	{
		// realloc memory
		let mapper = &proc_c ().addr_space;

		let virt_zone = match mapper.get_mapped_range (VirtAddr::new_truncate (addr as u64))
		{
			Some(range) => range,
			None => {
				vals.a1 = 0;
				vals.a2 = 0;
				vals.a3 = Realloc::InvlPointer as usize;
				return;
			}
		};

		unimplemented! ();
	}
}