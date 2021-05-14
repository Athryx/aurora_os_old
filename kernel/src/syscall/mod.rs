use crate::uses::*;
use crate::arch::x64::{rdmsr, wrmsr, EFER_MSR, EFER_SYSCALL_ENABLE, STAR_MSR, LSTAR_MSR, FMASK_MSR};
use crate::sched::sys::{thread_new, thread_block};

extern "C"
{
	fn syscall_entry ();
}

pub type SyscallFunc = extern "C" fn(&mut SyscallVals) -> ();

#[no_mangle]
static mut syscalls: [SyscallFunc; 16] = [
	sys_hi,
	sys_hi,
	sys_hi,
	sys_hi,
	sys_hi,
	sys_hi,
	sys_hi,
	sys_hi,
	sys_hi,
	sys_hi,
	sys_hi,
	sys_hi,
	sys_hi,
	sys_hi,
	sys_hi,
	sys_hi,
];

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct SyscallVals
{
	options: usize,
	a1: usize,
	a2: usize,
	a3: usize,
	a4: usize,
	a5: usize,
	a6: usize,
	a7: usize,
	a8: usize,
	a9: usize,
	a10: usize,
	rip: usize,
	rsp: usize,
	rflags: usize,
}

extern "C" fn sys_hi (vals: &mut SyscallVals)
{
	println! ("hi");
	eprintln! ("vals: {:#x?}", vals);
	eprintln! ("options {:x}", vals.options);
	eprintln! ("num {}", vals.a1);
	vals.a1 = 0x43;
	vals.a2 = 0x53;
}

pub fn init ()
{
	let efer = rdmsr (EFER_MSR);
	wrmsr (EFER_MSR, efer | EFER_SYSCALL_ENABLE);

	// tell cpu syscall instruction entry point
	wrmsr (LSTAR_MSR, syscall_entry as u64);

	// tell cpu to disable interrupts on syscall_entry
	wrmsr (FMASK_MSR, 0x200);

	// load correct segment values after syscall and sysret
	wrmsr (STAR_MSR, 0x0013000800000000);
}
