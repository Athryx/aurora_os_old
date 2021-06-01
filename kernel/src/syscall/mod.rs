use crate::uses::*;
use crate::arch::x64::{rdmsr, wrmsr, EFER_MSR, EFER_SYSCALL_ENABLE, STAR_MSR, LSTAR_MSR, FMASK_MSR};
use crate::sched::sys::{thread_new, thread_block};
use crate::mem::sys::realloc;

pub use sys_consts::SysErr;

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

#[macro_export]
macro_rules! sysret
{
	() => {
		return
	};

	($v:ident) => {
		return
	};

	($v:ident, $r1:expr) => {{
		$v.a1 = $r1;
		return;
	}};

	($v:ident, $r1:expr, $r2:expr) => {{
		$v.a1 = $r1;
		$v.a2 = $r2;
		return;
	}};

	($v:ident, $r1:expr, $r2:expr, $r3:expr) => {{
		$v.a1 = $r1;
		$v.a2 = $r2;
		$v.a3 = $r3;
		return
	}};

	($v:ident, $r1:expr, $r2:expr, $r3:expr, $r4:expr) => {{
		$v.a1 = $r1;
		$v.a2 = $r2;
		$v.a3 = $r3;
		$v.a4 = $r4;
		return;
	}};

	($v:ident, $r1:expr, $r2:expr, $r3:expr, $r4:expr, $r5:expr) => {{
		$v.a1 = $r1;
		$v.a2 = $r2;
		$v.a3 = $r3;
		$v.a4 = $r4;
		$v.a5 = $r5;
		return;
	}};

	($v:ident, $r1:expr, $r2:expr, $r3:expr, $r4:expr, $r5:expr, $r6:expr) => {{
		$v.a1 = $r1;
		$v.a2 = $r2;
		$v.a3 = $r3;
		$v.a4 = $r4;
		$v.a5 = $r5;
		$v.a6 = $r6;
		return;
	}};

	($v:ident, $r1:expr, $r2:expr, $r3:expr, $r4:expr, $r5:expr, $r6:expr, $r7:expr) => {{
		$v.a1 = $r1;
		$v.a2 = $r2;
		$v.a3 = $r3;
		$v.a4 = $r4;
		$v.a5 = $r5;
		$v.a6 = $r6;
		$v.a7 = $r7;
		return;
	}};

	($v:ident, $r1:expr, $r2:expr, $r3:expr, $r4:expr, $r5:expr, $r6:expr, $r7:expr, $r8:expr) => {{
		$v.a1 = $r1;
		$v.a2 = $r2;
		$v.a3 = $r3;
		$v.a4 = $r4;
		$v.a5 = $r5;
		$v.a6 = $r6;
		$v.a7 = $r7;
		$v.a8 = $r8;
		return;
	}};

	($v:ident, $r1:expr, $r2:expr, $r3:expr, $r4:expr, $r5:expr, $r6:expr, $r7:expr, $r8:expr, $r9:expr) => {{
		$v.a1 = $r1;
		$v.a2 = $r2;
		$v.a3 = $r3;
		$v.a4 = $r4;
		$v.a5 = $r5;
		$v.a6 = $r6;
		$v.a7 = $r7;
		$v.a8 = $r8;
		$v.a9 = $r9;
		return;
	}};

	($v:ident, $r1:expr, $r2:expr, $r3:expr, $r4:expr, $r5:expr, $r6:expr, $r7:expr, $r8:expr, $r9:expr, $r10:expr) => {{
		$v.a1 = $r1;
		$v.a2 = $r2;
		$v.a3 = $r3;
		$v.a4 = $r4;
		$v.a5 = $r5;
		$v.a6 = $r6;
		$v.a7 = $r7;
		$v.a8 = $r8;
		$v.a9 = $r9;
		$v.a10 = $r10;
		return;
	}};
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
