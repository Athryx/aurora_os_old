use crate::uses::*;
use sys_consts::options::RegOptions;
use crate::syscall::SyscallVals;
use crate::sysret;
use super::*;

pub extern "C" fn thread_new (vals: &mut SyscallVals)
{
	let rip = vals.a1;

	match proc_c ().new_thread (unsafe { core::mem::transmute (rip) }, None)
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

// TODO: handler reg_public and reg_group options
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
		handler_options.blocking_mode = BlockMode::Blocking(thread_res_c ().tid ());
	}

	let handler = DomainHandler::new (rip, handler_options);

	proc_c ().domains ().lock ().register (domain, handler);

	sysret! (vals, 0);
}
