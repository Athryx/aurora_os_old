use crate::uses::*;
use crate::syscall::SyscallVals;
use super::PAGE_SIZE;
use super::phys_alloc::zm;
use super::virt_alloc::{VirtLayoutElement, VirtLayout, PageTableFlags};
use crate::sched::proc_c;

const READ: u32 = 1;
const WRITE: u32 = 1 << 1;
const EXEC: u32 = 1 << 2;

const REALLOC_EXACT: usize = 1 << 4;

fn get_page_table_flags (options: u32)
{
	let mut out = PageTableFlags::NONE;
	if options & WRITE != 0
	{
		out |= PageTableFlags::WRITABLE;
	}

	if options & EXEC == 0
	{
		out |= PageTableFlags::NO_EXEC;
	}

	if options & READ != 0 || out.bits () != 0
	{
		out |= PageTableFlags::PRESENT;
	}

	out | PageTableFlags::USER;
}

pub extern "C" fn realloc (vals: &mut SyscallVals)
{
	let options = vals.options;
	let addr = vals.a1;
	let size = vals.a2;
	let at_addr = vals.a3;

	let flags = get_page_table_flags (options);

	if addr == 0
	{
		if size == 0
		{
			// no need to set values
			// they are already 0
			vals.a3 = 0;
			return;
		}

		let pmem = match zm.alloc (size * PAGE_SIZE)
		{
			Some(allocation) => allocation,
			None => {
				vals.a2 = 1;
				return;
			}
		};

		let vec = vec![VirtLayoutElement::AllocedMem (pmem)];

		let layout = VirtLayout::new (vec);

		let mapper = proc_c ().addr_space;

		mapper.map
	}
}
