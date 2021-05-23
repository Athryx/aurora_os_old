use crate::uses::*;
use crate::arch::x64::{rdmsr, wrmsr, EFER_MSR, EFER_SYSCALL_ENABLE, STAR_MSR, LSTAR_MSR, FMASK_MSR};
use crate::sched::sys::{thread_new, thread_block};
use crate::mem::sys::realloc;

pub mod consts;

extern "C"
{
	fn syscall_entry ();
}

pub type SyscallFunc = extern "C" fn(&mut SyscallVals) -> ();

#[no_mangle]
static syscalls: [SyscallFunc; 16] = [
	sys_hi,
	sys_hi,
	thread_new,
	thread_block,
	sys_hi,
	sys_hi,
	sys_hi,
	sys_hi,
	sys_hi,
	sys_hi,
	sys_hi,
	realloc,
	sys_hi,
	sys_hi,
	sys_hi,
	sys_hi,
];

// TODO: figure out if packed is needed
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SyscallVals
{
	pub options: u32,
	unused: u32,
	pub a1: usize,
	pub a2: usize,
	pub a3: usize,
	pub a4: usize,
	pub a5: usize,
	pub a6: usize,
	pub a7: usize,
	pub a8: usize,
	pub a9: usize,
	pub a10: usize,
	pub rip: usize,
	pub rsp: usize,
	pub rflags: usize,
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
