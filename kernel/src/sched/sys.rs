use crate::uses::*;
use sys_consts::options::{ConnectOptions, RegOptions};
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

// TODO: handle reg_group option
pub extern "C" fn reg (vals: &mut SyscallVals)
{
	let options = RegOptions::from_bits_truncate (vals.options);
	let rip = vals.a2;
	let domain = if options.contains (RegOptions::DEFAULT)
	{
		None
	}
	else
	{
		Some(vals.a1)
	};

	let block_mode = if options.contains (RegOptions::BLOCK)
	{
		BlockMode::Blocking(thread_c ().tid ())
	}
	else
	{
		BlockMode::NonBlocking
	};
	let public = options.contains (RegOptions::PUBLIC);

	let handler_options = HandlerOptions::new (block_mode, public);
	let process = proc_c ();
	let pid = process.pid ();
	let handler = DomainHandler::new (rip, pid, handler_options);

	let remove = options.contains (RegOptions::REMOVE);

	if options.contains (RegOptions::GLOBAL)
	{
		let mut dmap = process.namespace ().domains ().lock ();

		if remove
		{
			if !dmap.remove (pid, domain)
			{
				sysret! (vals, SysErr::InvlPriv.num ());
			}
		}
		else if !dmap.register (pid, domain, handler)
		{
			sysret! (vals, SysErr::InvlPriv.num ());
		}
	}
	else
	{
		// don't need to check if fail, because no other processes will insert into this one
		let mut dmap = process.domains ().lock ();
		if remove
		{
			dmap.remove (pid, domain);
		}
		else
		{
			dmap.register (pid, domain, handler);
		}
	}

	sysret! (vals, SysErr::Ok.num ());
}

pub extern "C" fn connect (vals: &mut SyscallVals)
{
	let options = ConnectOptions::from_bits_truncate (vals.options);
	let domain = vals.a3;

	let handler = if options.contains (ConnectOptions::PID)
	{
		let pid = vals.a1;
		let plock = proc_list.lock ();
		let process = match plock.get (&pid)
		{
			Some(process) => process,
			None => sysret! (vals, SysErr::InvlId.num (), 0),
		};
		let dlock = process.domains ().lock ();
		match dlock.get (domain)
		{
			Some(handler) => *handler,
			None => sysret! (vals, SysErr::InvlId.num (), 0),
		}
	}
	else
	{
		let ustring = UserString::from_parts (vals.a1 as *const u8, vals.a2);
		let exec_name = match ustring.try_fetch ()
		{
			Some(string) => string,
			None => sysret! (vals, SysErr::InvlString.num (), 0),
		};
		let namespace = match namespace_map.lock ().get (&exec_name)
		{
			// should be ok to unwrap because we hold lock, and namespaces ensure that when dropped it removes the weak from namespace_map
			Some(namespace) => namespace.upgrade ().unwrap (),
			None => sysret! (vals, SysErr::InvlId.num (), 0),
		};
		let dlock = namespace.domains ().lock ();
		match dlock.get (domain)
		{
			Some(handler) => *handler,
			None => sysret! (vals, SysErr::InvlId.num (), 0),
		}
	};

	let process = proc_c ();
	let plock = proc_list.lock ();
	let other_process = match plock.get (&handler.pid ())
	{
		Some(process) => process,
		None => sysret! (vals, SysErr::InvlId.num (), 0),
	};

	if process.pid () == other_process.pid ()
	{
		sysret! (vals, SysErr::InvlId.num (), 0);
	}

	let connection = Connection::new (domain, handler, process.pid ());

	// NOTE: this doesn't keep locks of both connection maps in each process locked, because this could be a race condition
	// as a result, msg could be called with a valid conn_id before connect returns, but this id would be invalid in the other process
	// so msg needs to check if get_ext on connection map in other process returns none, than it should return InvlId
	let int_id = process.insert_connection (connection.clone ());
	let ext_id = other_process.insert_connection (connection);
	process.assoc_connection (int_id, ext_id);
	other_process.assoc_connection (ext_id, int_id);

	sysret! (vals, SysErr::Ok.num (), int_id);
}

pub extern "C" fn disconnect (vals: &mut SyscallVals)
{
}

pub extern "C" fn conn_info (vals: &mut SyscallVals)
{
}

// TODO: handler smem_transfer_mask options
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
