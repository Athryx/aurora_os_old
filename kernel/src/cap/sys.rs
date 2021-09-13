use crate::uses::*;
use crate::sysret;
use crate::syscall::SyscallVals;
use crate::mem::PAGE_SIZE;
use crate::sched::proc_c;
use super::{CapId, CapFlags, CapObjectType};

pub extern "C" fn cap_destroy(vals: &mut SyscallVals) {
	let id = CapId::from(vals.a1);

	let err = if proc_c().get_capmap(id.cap_object_type()).destroy(id) {
		SysErr::Ok
	} else {
		SysErr::InvlId
	};

	sysret!(vals, err.num());
}

pub extern "C" fn cap_clone(vals: &mut SyscallVals) {
	let id = CapId::from(vals.a1);
	let flags = CapFlags::from_bits_truncate(vals.options as usize);

	match proc_c().get_capmap(id.cap_object_type()).clone_cap(id, flags) {
		Some(id) => sysret!(vals, SysErr::Ok.num(), id.into()),
		None => sysret!(vals, SysErr::InvlId.num(), 0),
	}
}

pub extern "C" fn cap_map(vals: &mut SyscallVals) {
	let id = CapId::from(vals.a1);
	let at_addr = if vals.a2 == 0 {
		None
	} else {
		Some(vals.a2)
	};

	let out = match id.cap_object_type() {
		CapObjectType::SMem => proc_c().smem().map(id, at_addr),
		CapObjectType::Mmio => todo!(),
		_ => Err(SysErr::InvlId),
	};

	match out {
		Ok(vrange) => sysret!(vals, SysErr::Ok.num(), vrange.as_usize(), vrange.size() / PAGE_SIZE),
		Err(err) => sysret!(vals, err.num(), 0, 0),
	}
}

pub extern "C" fn cap_unmap(vals: &mut SyscallVals) {
	let id = CapId::from(vals.a1);

	let out = match id.cap_object_type() {
		CapObjectType::SMem => proc_c().smem().unmap(id),
		CapObjectType::Mmio => todo!(),
		_ => Err(SysErr::InvlId),
	};

	match out {
		Ok(()) => sysret!(vals, SysErr::Ok.num()),
		Err(err) => sysret!(vals, err.num()),
	}
}

pub extern "C" fn cap_info(vals: &mut SyscallVals) {
	todo!();
}
