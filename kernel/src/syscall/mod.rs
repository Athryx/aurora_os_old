use crate::uses::*;
use crate::arch::x64::{rdmsr, wrmsr, EFER_MSR, EFER_SYSCALL_ENABLE, STAR_MSR, LSTAR_MSR, FMASK_MSR};
use crate::sched::sys::{thread_new, thread_block};

extern "C"
{
	fn syscall_entry ();
}

#[no_mangle]
static mut syscalls: [usize; 16] = [0; 16];

extern "C" fn sys_hi (vals: &SyscallVals, options: u32, num: usize) -> usize
{
	println! ("hi");
	eprintln! ("vals: {:#x?}", vals);
	eprintln! ("options {:x}", options);
	eprintln! ("num {}", num);
	0x43
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct SyscallVals
{
	rip: usize,
	rsp: usize,
	rflags: usize,
}

pub fn reg_syscall_handler (syscall: u32, addr: usize)
{
	unsafe
	{
		syscalls[syscall as usize] = addr;
	}
}

pub fn init ()
{
	let efer = rdmsr (EFER_MSR);
	wrmsr (EFER_MSR, efer | EFER_SYSCALL_ENABLE);

	// tell cpu syscall instruction entry point
	wrmsr (LSTAR_MSR, syscall_entry as u64);

	// tell cpu to disable interrupts on syscall_entry
	wrmsr (FMASK_MSR, 0x200);
	reg_syscall_handler (0, sys_hi as usize);
	reg_syscall_handler (2, thread_new as usize);
	reg_syscall_handler (3, thread_block as usize);
}
