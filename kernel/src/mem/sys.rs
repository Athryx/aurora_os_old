use crate::uses::*;
use sys_consts::options::*;
use crate::sysret;
use crate::syscall::{SyscallVals, SysErr};
use super::{PAGE_SIZE, VirtRange};
use super::phys_alloc::zm;
use super::virt_alloc::{VirtLayoutElement, VirtLayout, PageMappingFlags, AllocType};
use super::shared_mem::*;
use super::error::MemErr;
use crate::sched::proc_c;

const READ: u32 = 1;
const WRITE: u32 = 1 << 1;
const EXEC: u32 = 1 << 2;

const REALLOC_EXACT: usize = 1 << 4;

// FIXME: this doesn't return the right error codes yet
// FIXME: this doesn't obey REALLOC_EXACT when decreasing size of virtual memory
pub extern "C" fn realloc (vals: &mut SyscallVals)
{
	let options = vals.options;
	let addr = vals.a1;
	let size = vals.a2 * PAGE_SIZE;
	let at_addr = vals.a3;

	if align_of (addr) < PAGE_SIZE || align_of (at_addr) < PAGE_SIZE
	{
		sysret! (vals, SysErr::InvlPtr.num (), 0, 0);
	}

	let at_vaddr = match VirtAddr::try_new (at_addr as u64)
	{
		Ok(addr) => addr,
		Err(_) => sysret! (vals, SysErr::InvlVirtAddr.num (), 0, 0),
	};

	let flags = PageMappingFlags::from_bits_truncate (options as usize)
		| PageMappingFlags::USER;

	if addr == 0
	{
		// allocate memory
		if size == 0
		{
			sysret! (vals, SysErr::Ok.num (), 0, 0);
		}

		let layout_element = match VirtLayoutElement::new (size, flags)
		{
			Some(elem) => elem,
			None => sysret! (vals, SysErr::OutOfMem.num (), 0, 0),
		};

		let vec = vec![layout_element];

		let layout = VirtLayout::from (vec, AllocType::VirtMem);

		if at_addr == 0
		{
			unsafe
			{
				match proc_c ().addr_space.map (layout)
				{
					Ok(virt_range) => sysret! (vals, SysErr::Ok.num (), virt_range.as_usize (), virt_range.size () / PAGE_SIZE),
					Err(_) => sysret! (vals, SysErr::OutOfMem.num (), 0, 0),
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
					Ok(virt_range) => sysret! (vals, SysErr::Ok.num (), virt_range.as_usize (), virt_range.size () / PAGE_SIZE),
					Err(_) => sysret! (vals, SysErr::OutOfMem.num (), 0, 0),
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
			None => sysret! (vals, SysErr::InvlPtr.num (), 0, 0),
		};

		match unsafe { mapper.unmap (virt_zone, AllocType::VirtMem) }
		{
			Ok(layout) => {
				unsafe { layout.dealloc () };
				sysret! (vals, SysErr::Ok.num (), 0, 0);
			},
			Err(_) => sysret! (vals, SysErr::Unknown.num (), 0, 0),
		}
	}
	else
	{
		// realloc memory
		let mapper = &proc_c ().addr_space;

		let virt_zone = match mapper.get_mapped_range (VirtAddr::new_truncate (addr as u64))
		{
			Some(range) => range,
			None => sysret! (vals, SysErr::InvlPtr.num (), 0, 0),
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
					.ok_or (MemErr::OutOfMem ("out of memory"))?;
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
				match proc_c ().addr_space.remap (virt_zone, AllocType::VirtMem, realloc_func)
				{
					Ok(virt_range) => sysret! (vals, SysErr::Ok.num (), virt_range.as_usize (), virt_range.size () / PAGE_SIZE),
					Err(_) => sysret! (vals, SysErr::OutOfMem.num (), 0, 0),
				}
			}
		}
		else
		{
			unsafe
			{
				match proc_c ().addr_space.remap_at (virt_zone, at_vaddr, AllocType::VirtMem, realloc_func)
				{
					Ok(virt_range) => sysret! (vals, SysErr::Ok.num (), virt_range.as_usize (), virt_range.size () / PAGE_SIZE),
					Err(_) => sysret! (vals, SysErr::OutOfMem.num (), 0, 0),
				}
			}
		}
	}
}

pub extern "C" fn mprotect (vals: &mut SyscallVals)
{
}

pub extern "C" fn salloc (vals: &mut SyscallVals)
{
	let size = vals.a1 * PAGE_SIZE;
	let options = SMemFlags::from_bits_truncate (vals.options as u8);

	let smem = match SharedMem::new (size, options)
	{
		Some(smem) => smem,
		None => sysret! (vals, SysErr::OutOfMem.num (), 0),
	};

	let smid = proc_c ().smem ().lock ().insert (smem);
	sysret! (vals, SysErr::Ok.num (), smid);
}

pub extern "C" fn sdealloc (vals: &mut SyscallVals)
{
	let smid = vals.a1;
	if let None = proc_c ().remove_smem (smid)
	{
		sysret! (vals, SysErr::InvlId.num ());
	}
	else
	{
		sysret! (vals, SysErr::Ok.num ());
	}
}

pub extern "C" fn smap (vals: &mut SyscallVals)
{
	let smid = vals.a1;
	match proc_c ().map_smem (smid)
	{
		Ok(vmem) => sysret! (vals, SysErr::Ok.num (), vmem.as_usize (), vmem.size () / PAGE_SIZE),
		Err(err) => sysret! (vals, err.num (), 0, 0),
	}
}

pub extern "C" fn sunmap (vals: &mut SyscallVals)
{
	let smid = vals.a1;
	sysret! (vals, proc_c ().unmap_smem (smid).num ());
}

pub extern "C" fn smem_info (vals: &mut SyscallVals)
{
}
