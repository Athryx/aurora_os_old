use crate::uses::*;
use crate::sysret;
use crate::syscall::{SyscallVals, consts};
use super::{PAGE_SIZE, VirtRange};
use super::phys_alloc::zm;
use super::virt_alloc::{VirtLayoutElement, VirtLayout, PageMappingFlags};
use crate::sched::proc_c;

const READ: u32 = 1;
const WRITE: u32 = 1 << 1;
const EXEC: u32 = 1 << 2;

const REALLOC_EXACT: usize = 1 << 4;

// TODO: use only 1 enum for all syscall error values
#[repr(usize)]
enum Realloc
{
	Ok = 0,
	OutOfMem = 1,
	AllocAtInvlAddr = 2,
	InvlPointer = 3,
	InvlVirtAddr = 4,
	Unknown = 5,
}

// FIXME: this doesn't return the right error codes yet
// FIXME: this doesn't obey REALLOC_EXACT
pub extern "C" fn realloc (vals: &mut SyscallVals)
{
	let options = vals.options;
	let addr = align_down (vals.a1, PAGE_SIZE);
	let size = vals.a2 * PAGE_SIZE;
	let at_addr = align_down (vals.a3, PAGE_SIZE);
	let at_vaddr = match VirtAddr::try_new (at_addr as u64)
	{
		Ok(addr) => addr,
		Err(_) => sysret! (vals, 0, 0, Realloc::InvlVirtAddr as usize),
	};

	let flags = PageMappingFlags::from_bits_truncate (options as usize)
		| PageMappingFlags::USER;

	if addr == 0
	{
		// allocate memory
		if size == 0
		{
			sysret! (vals, 0, 0, Realloc::Ok as usize);
		}

		let layout_element = match VirtLayoutElement::new (size, flags)
		{
			Some(elem) => elem,
			None => sysret! (vals, 0, 0, Realloc::OutOfMem as usize),
		};

		let vec = vec![layout_element];

		let layout = VirtLayout::from (vec);

		if at_addr == 0
		{
			unsafe
			{
				match proc_c ().addr_space.map (layout)
				{
					Ok(virt_range) => sysret! (vals, virt_range.as_usize (), virt_range.size () / PAGE_SIZE, 0),
					Err(_) => sysret! (vals, 0, 0, Realloc::OutOfMem as usize),
				}
			}
		}
		else
		{
			let virt_zone = VirtRange::new (VirtAddr::new_truncate (at_addr as u64), layout.size ());
			unsafe
			{
				match proc_c ().addr_space.map_at (layout, virt_zone)
				{
					Ok(virt_range) => sysret! (vals, virt_range.as_usize (), virt_range.size () / PAGE_SIZE, 0),
					Err(_) => sysret! (vals, 0, 0, Realloc::OutOfMem as usize),
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
			None => sysret! (vals, 0, 0, Realloc::InvlPointer as usize),
		};

		match unsafe { mapper.unmap (virt_zone) }
		{
			Ok(layout) => {
				unsafe { layout.dealloc () };
				sysret! (vals, 0, 0, 0);
			},
			Err(_) => sysret! (vals, 0, 0, Realloc::Unknown as usize),
		}
	}
	else
	{
		// realloc memory
		let mapper = &proc_c ().addr_space;

		let virt_zone = match mapper.get_mapped_range (VirtAddr::new_truncate (addr as u64))
		{
			Some(range) => range,
			None => sysret! (vals, 0, 0, Realloc::InvlPointer as usize),
		};

		// closure type annotation needed, compiler complains for some reason if I don't have it
		// TODO: obey exact size flag here
		let realloc_func = |phys_zones: &mut VirtLayout| {
			let psize = phys_zones.size ();
			let mut new_flags = phys_zones.flags ().unwrap ();
			if flags.contains (PageMappingFlags::EXACT_SIZE)
			{
				new_flags |= PageMappingFlags::EXACT_SIZE;
			}

			if size > psize
			{
				let elem = VirtLayoutElement::new (size - psize, new_flags)
					.ok_or_else (|| Err::new ("out of memory"))?;
				phys_zones.push (elem);
			}
			else if size < psize
			{
				let mut diff = psize - size;

				while let Some(a) = phys_zones.clean_slice ().last ()
				{
					if diff > a.size ()
					{
						diff -= a.size ();
						phys_zones.pop_delete ();
					}
				}
			}

			Ok(())
		};

		if at_addr == 0
		{
			unsafe
			{
				match proc_c ().addr_space.remap (virt_zone, realloc_func)
				{
					Ok(virt_range) => sysret! (vals, virt_range.as_usize (), virt_range.size () / PAGE_SIZE, 0),
					Err(_) => sysret! (vals, 0, 0, Realloc::OutOfMem as usize),
				}
			}
		}
		else
		{
			unsafe
			{
				match proc_c ().addr_space.remap_at (virt_zone, at_vaddr, realloc_func)
				{
					Ok(virt_range) => sysret! (vals, virt_range.as_usize (), virt_range.size () / PAGE_SIZE, 0),
					Err(_) => sysret! (vals, 0, 0, Realloc::OutOfMem as usize),
				}
			}
		}
	}
}
