use crate::uses::*;
use sys_consts::options::{RegOptions};
use crate::syscall::{SyscallVals, udata::{UserData, UserArray, UserString, fetch_data}};
use crate::util::copy_to_heap;
use crate::sysret;
use super::*;

#[derive(Debug, Clone, Copy)]
struct ProcStartData
{
	name: UserString,
	uid: usize,
	args: UserArray<UserString>,
}

unsafe impl UserData for ProcStartData {}

// FIXME: make sure uid is valid once uid system is added to kernel
// FIXME: use args
pub extern "C" fn spawn (vals: &mut SyscallVals)
{
	if proc_c ().uid () != PrivLevel::SuperUser
	{
		sysret! (vals, 0, SysErr::InvlPriv.num ());
	}

	let elf_arr = UserArray::from_parts (vals.a1 as *const u8, vals.a2);

	let psdata_ptr = vals.a3 as *const ProcStartData;
	let psdata = match fetch_data (psdata_ptr)
	{
		Some(data) => data,
		None => sysret! (vals, 0, SysErr::InvlPtr.num ()),
	};

	let name = match psdata.name.try_fetch ()
	{
		Some(name) => name,
		None => sysret! (vals, 0, SysErr::InvlPtr.num ()),
	};

	let elf_data = match elf_arr.try_fetch ()
	{
		Some(data) => data,
		None => sysret! (vals, 0, SysErr::InvlPtr.num ()),
	};

	let process = match Process::from_elf (&elf_data, PrivLevel::new (psdata.uid), name)
	{
		Ok(process) => process,
		Err(_) => sysret! (vals, 0, SysErr::Unknown.num ()),
	};

	sysret! (vals, process.pid (), SysErr::Ok.num ());
}

pub extern "C" fn thread_new (vals: &mut SyscallVals)
{
	let rip = vals.a1;

	match proc_c ().new_thread (rip, None)
	{
		Ok(tid) => {
			sysret! (vals, tid, 0);
		},
		Err(_) => {
			sysret! (vals, 0, 1);
		}
	}
}

pub extern "C" fn thread_block (vals: &mut SyscallVals)
{
	let reason = vals.a1;
	let arg = vals.a2;

	match reason
	{
		0 => thread_c ().block (ThreadState::Running),
		1 => thread_c ().block (ThreadState::Destroy),
		2 => thread_c ().block (ThreadState::Sleep(arg as u64)),
		3 => thread_c ().block (ThreadState::Join (arg)),
		_ => (),
	}
}

pub extern "C" fn futex_block (vals: &mut SyscallVals)
{
	let addr = vals.a1;
	thread_c ().block (ThreadState::FutexBlock(addr));
}

pub extern "C" fn futex_unblock (vals: &mut SyscallVals)
{
	let addr = vals.a1;
	let n = vals.a2;
	vals.a1 = proc_c ().futex_move (addr, ThreadState::Running, n);
}

pub extern "C" fn futex_move (vals: &mut SyscallVals)
{
	let addr_old = vals.a1;
	let addr_new = vals.a2;
	let n = vals.a3;
	vals.a1 = proc_c ().futex_move (addr_old, ThreadState::FutexBlock(addr_new), n);
}

// TODO: handle reg_public and reg_group options
pub extern "C" fn reg (vals: &mut SyscallVals)
{
	let options = RegOptions::from_bits_truncate (vals.options);
	let domain = vals.a1;
	let rip = vals.a2;

	let mut handler_options = HandlerOptions::new ();

	let domain = if options.contains (RegOptions::DEFAULT)
	{
		None
	}
	else
	{
		Some(domain)
	};

	if options.contains (RegOptions::BLOCK)
	{
		handler_options.blocking_mode = BlockMode::Blocking(thread_c ().tid ());
	}

	let handler = DomainHandler::new (rip, handler_options);

	proc_c ().domains ().lock ().register (domain, handler);

	sysret! (vals, 0);
}

// TODO: handler msg_pid and smem_transfer_mask options
pub extern "C" fn msg (vals: &mut SyscallVals)
{
	match connection::msg (vals)
	{
		Ok(regs) => {
			vals.options = regs.rax as u32;
			vals.a1 = regs.rbx;
			vals.a2 = regs.rdx;
			vals.a3 = regs.rsi;
			vals.a4 = regs.rdi;
			vals.a5 = regs.r8;
			vals.a6 = regs.r9;
			vals.a7 = regs.r12;
			vals.a8 = regs.r13;
			vals.a9 = regs.r14;
			vals.a10 = regs.r15;
			vals.rsp = regs.rsp;
			vals.rip = regs.rip;
		},
		Err(msg_err) => {
			vals.options = msg_err.num () as u32;
		},
	}
}
