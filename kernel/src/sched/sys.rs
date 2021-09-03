use sys_consts::options::{ConnectOptions, FutexOptions, RegOptions};

use crate::uses::*;
use crate::syscall::udata::{fetch_data, UserArray, UserData, UserString};
use crate::syscall::SyscallVals;
use crate::sysret;
use crate::mem::PAGE_SIZE;
use super::*;

// FIXME: make sure uid is valid once uid system is added to kernel
pub extern "C" fn spawn(vals: &mut SyscallVals)
{
	if proc_c().uid() != PrivLevel::SuperUser {
		sysret!(vals, SysErr::InvlPriv.num(), 0);
	}

	let name = UserString::from_parts(vals.a1 as *const u8, vals.a2);
	let uid = vals.a3;

	let spawn_state = vals.a7 as *const SpawnStartState;
	let spawn_state = match fetch_data(spawn_state) {
		Some(data) => data,
		None => sysret!(vals, SysErr::InvlPtr.num(), 0),
	};

	let name = match name.try_fetch() {
		Some(name) => name,
		None => sysret!(vals, SysErr::InvlPtr.num(), 0),
	};

	let process = match Process::spawn(PrivLevel::new(uid), name, spawn_state) {
		Ok(process) => process,
		Err(err) => sysret!(vals, err.num(), 0),
	};

	sysret!(vals, SysErr::Ok.num(), process.pid());
}

pub extern "C" fn thread_new(vals: &mut SyscallVals)
{
	let rip = vals.a1;

	match proc_c().new_thread(rip, None) {
		Ok(tid) => {
			sysret!(vals, SysErr::Ok.num(), tid);
		},
		Err(_) => {
			sysret!(vals, SysErr::Unknown.num(), 0);
		},
	}
}

pub extern "C" fn thread_block(vals: &mut SyscallVals)
{
	let reason = vals.a1;
	let arg = vals.a2;

	match reason {
		0 => thread_c().block(ThreadState::Running),
		1 => thread_c().block(ThreadState::Destroy),
		2 => thread_c().block(ThreadState::Sleep(arg as u64)),
		3 => thread_c().block(ThreadState::Join(Tuid::new(proc_c().pid(), arg))),
		_ => sysret!(vals, SysErr::InvlArgs.num()),
	}

	sysret!(vals, SysErr::Ok.num());
}

// TODO: handle timeout option
// TODO: merge common code of futex syscalls together when cleanup kernel code
pub extern "C" fn futex_block(vals: &mut SyscallVals)
{
	let options = FutexOptions::from_bits_truncate(vals.options);
	let id = vals.a1;

	let process = proc_c();

	if options.contains(FutexOptions::SHARE) {
		let smid = vals.a2;
		let slock = process.smem().lock();
		let smem_entry = match slock.get(smid) {
			Some(smem) => smem,
			None => sysret!(vals, SysErr::InvlId.num()),
		};
		smem_entry.smem().futex().block(id);
	} else {
		process.futex().block(id);
	}

	sysret!(vals, SysErr::Ok.num());
}

pub extern "C" fn futex_unblock(vals: &mut SyscallVals)
{
	let options = FutexOptions::from_bits_truncate(vals.options);
	let id = vals.a1;
	let n = vals.a3;

	let process = proc_c();

	let out = if options.contains(FutexOptions::SHARE) {
		let smid = vals.a2;
		let slock = process.smem().lock();
		let smem_entry = match slock.get(smid) {
			Some(smem) => smem,
			None => sysret!(vals, SysErr::InvlId.num()),
		};
		smem_entry.smem().futex().unblock(id, n)
	} else {
		process.futex().unblock(id, n)
	};

	sysret!(vals, SysErr::Ok.num(), out);
}

pub extern "C" fn futex_destroy(vals: &mut SyscallVals)
{
	let options = FutexOptions::from_bits_truncate(vals.options);
	let id = vals.a1;

	let process = proc_c();

	let out = if options.contains(FutexOptions::SHARE) {
		let smid = vals.a2;
		let slock = process.smem().lock();
		let smem_entry = match slock.get(smid) {
			Some(smem) => smem,
			None => sysret!(vals, SysErr::InvlId.num()),
		};
		smem_entry.smem().futex().destroy(id)
	} else {
		process.futex().destroy(id)
	};

	match out {
		Ok(n) => sysret!(vals, SysErr::Ok.num(), n),
		Err(err) => sysret!(vals, err.num(), 0),
	}
}

// TODO: handle reg_group option
pub extern "C" fn reg(vals: &mut SyscallVals)
{
	let options = RegOptions::from_bits_truncate(vals.options);
	let rip = vals.a2;
	let domain = if options.contains(RegOptions::DEFAULT) {
		None
	} else {
		Some(vals.a1)
	};

	let block_mode = if options.contains(RegOptions::BLOCK) {
		BlockMode::Blocking(thread_c().tid())
	} else {
		BlockMode::NonBlocking
	};
	let public = options.contains(RegOptions::PUBLIC);

	let handler_options = HandlerOptions::new(block_mode, public);
	let process = proc_c();
	let pid = process.pid();
	let handler = DomainHandler::new(rip, pid, handler_options);

	let remove = options.contains(RegOptions::REMOVE);

	if options.contains(RegOptions::GLOBAL) {
		let mut dmap = process.namespace().domains().lock();

		if remove {
			if !dmap.remove(pid, domain) {
				sysret!(vals, SysErr::InvlPriv.num());
			}
		} else if !dmap.register(pid, domain, handler) {
			sysret!(vals, SysErr::InvlPriv.num());
		}
	} else {
		// don't need to check if fail, because no other processes will insert into this one
		let mut dmap = process.domains().lock();
		if remove {
			dmap.remove(pid, domain);
		} else {
			dmap.register(pid, domain, handler);
		}
	}

	sysret!(vals, SysErr::Ok.num());
}

pub extern "C" fn connect(vals: &mut SyscallVals)
{
	let options = ConnectOptions::from_bits_truncate(vals.options);
	let domain = vals.a3;

	let handler = if options.contains(ConnectOptions::PID) {
		let pid = vals.a1;
		let plock = proc_list.lock();
		let process = match plock.get(&pid) {
			Some(process) => process,
			None => sysret!(vals, SysErr::InvlId.num(), 0),
		};
		let dlock = process.domains().lock();
		match dlock.get(domain) {
			Some(handler) => *handler,
			None => sysret!(vals, SysErr::InvlId.num(), 0),
		}
	} else {
		let ustring = UserString::from_parts(vals.a1 as *const u8, vals.a2);
		let exec_name = match ustring.try_fetch() {
			Some(string) => string,
			None => sysret!(vals, SysErr::InvlString.num(), 0),
		};
		let namespace = match namespace_map.lock().get(&exec_name) {
			// should be ok to unwrap because we hold lock, and namespaces ensure that when dropped it removes the weak from namespace_map
			Some(namespace) => namespace.upgrade().unwrap(),
			None => sysret!(vals, SysErr::InvlId.num(), 0),
		};
		let dlock = namespace.domains().lock();
		match dlock.get(domain) {
			Some(handler) => *handler,
			None => sysret!(vals, SysErr::InvlId.num(), 0),
		}
	};

	if !handler.options().public {
		sysret!(vals, SysErr::InvlId.num(), 0);
	}

	let process = proc_c();
	let plock = proc_list.lock();
	let other_process = match plock.get(&handler.pid()) {
		Some(process) => process,
		None => sysret!(vals, SysErr::InvlId.num(), 0),
	};

	if process.pid() == other_process.pid() {
		sysret!(vals, SysErr::InvlId.num(), 0);
	}

	let cid = process.connections().lock().next_id();
	let other_cid = other_process.connections().lock().next_id();

	let cpid = ConnPid::new(process.pid(), cid);
	let other_cpid = ConnPid::new(other_process.pid(), other_cid);

	let connection = Connection::new(domain, handler, cpid, other_cpid);

	// NOTE: this doesn't keep locks of both connection maps in each process locked, because this could be a race condition
	// as a result, msg could be called with a valid conn_id before connect returns, but this id would be invalid in the other process
	// so msg needs to check if get_ext on connection map in other process returns none, than it should return InvlId
	process.insert_connection(connection.clone());
	other_process.insert_connection(connection);

	sysret!(vals, SysErr::Ok.num(), cid);
}

pub extern "C" fn disconnect(vals: &mut SyscallVals)
{
	let cid = vals.a1;
	let process = proc_c();
	let connection = match process.connections().lock().remove(cid) {
		Some(connection) => connection,
		None => sysret!(vals, SysErr::InvlId.num()),
	};

	let cpid = connection.other(process.pid());
	let plock = proc_list.lock();
	if let Some(process) = plock.get(&cpid.pid()) {
		process.connections().lock().remove(cpid.conn_id());
	}

	sysret!(vals, SysErr::Ok.num());
}

pub extern "C" fn conn_info(vals: &mut SyscallVals) {}

// TODO: handler smem_transfer_mask options
pub extern "C" fn msg(vals: &mut SyscallVals)
{
	match connection::msg(vals) {
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
			vals.options = msg_err.num() as u32;
		},
	}
}

pub extern "C" fn msg_return(vals: &mut SyscallVals)
{
	match thread_c().pop_conn_state() {
		Ok(_) => sysret!(vals, SysErr::Ok.num()),
		Err(err) => sysret!(vals, err.num()),
	}
}
