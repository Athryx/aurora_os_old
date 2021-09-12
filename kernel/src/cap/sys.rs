use crate::uses::*;
use crate::syscall::SyscallVals;
use crate::sched::proc_c;
use super::{CapId, CapObjectType};

pub extern "C" fn cap_destroy(vals: &mut SyscallVals) {
	let id = CapId::from(vals.a1);
}
