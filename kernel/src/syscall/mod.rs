use crate::uses::*;
use crate::arch::x64::{rdmsr, wrmsr, EFER_MSR, EFER_SYSCALL_ENABLE, STAR_MSR, LSTAR_MSR, FMASK_MSR};
use crate::sched::sys::{spawn, thread_new, thread_block, futex_block, futex_unblock, futex_move, reg, connect, disconnect, conn_info, msg, msg_return};
use crate::mem::sys::realloc;
use crate::util::io::sys_print_debug;

pub use sys_consts::SysErr;

pub mod udata;

extern "C"
{
	fn syscall_entry ();
}

pub type SyscallFunc = extern "C" fn(&mut SyscallVals) -> ();

#[no_mangle]
static syscalls: [SyscallFunc; 29] = [
	sys_nop,
	spawn,
	thread_new,
	thread_block,
	sys_nop,
	sys_nop,
	futex_block,
	futex_unblock,
	futex_move,
	sys_nop,
	sys_nop,
	realloc,
	sys_nop,
	sys_nop,
	sys_nop,
	sys_nop,
	sys_nop,
	sys_nop,
	sys_nop,
	sys_nop,
	sys_nop,
	sys_nop,
	reg,
	connect,
	disconnect,
	conn_info,
	msg,
	msg_return,
	sys_print_debug,
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

extern "C" fn sys_nop (_: &mut SyscallVals)
{
}

pub fn init ()
{
	let efer = rdmsr (EFER_MSR);
	wrmsr (EFER_MSR, efer | EFER_SYSCALL_ENABLE);

	// tell cpu syscall instruction entry point
	wrmsr (LSTAR_MSR, syscall_entry as usize as u64);

	// tell cpu to disable interrupts on syscall_entry
	wrmsr (FMASK_MSR, 0x200);

	// load correct segment values after syscall and sysret
	wrmsr (STAR_MSR, 0x0013000800000000);
}
