use sys_consts::options::{ConnectOptions, FutexOptions, RegOptions};

use crate::uses::*;
use crate::syscall::udata::{fetch_data, UserArray, UserData, UserString};
use crate::syscall::SyscallVals;
use crate::sysret;
use crate::mem::PAGE_SIZE;
use crate::cap::CapId;
use super::*;

// FIXME: make sure uid is valid once uid system is added to kernel
pub extern "C" fn spawn(vals: &mut SyscallVals)
{
	if proc_c().uid() != PrivLevel::SuperUser {
		sysret!(vals, SysErr::InvlPriv.num(), 0);
	}

	let name = UserString::from_parts(vals.a1 as *const u8, vals.a2);
	let launch_path = UserString::from_parts(vals.a3 as *const u8, vals.a4);
	let uid = vals.a5;

	let spawn_state = vals.a6 as *const SpawnStartState;
	let spawn_state = match fetch_data(spawn_state) {
		Some(data) => data,
		None => sysret!(vals, SysErr::InvlPtr.num(), 0),
	};

	let name = match name.try_fetch() {
		Some(name) => name,
		None => sysret!(vals, SysErr::InvlPtr.num(), 0),
	};

	let launch_path = match launch_path.try_fetch() {
		Some(name) => name,
		None => sysret!(vals, SysErr::InvlPtr.num(), 0),
	};

	let process = match Process::spawn(PrivLevel::new(uid), name, launch_path, spawn_state) {
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

pub extern "C" fn futex_new(vals: &mut SyscallVals) {
	let futex = KFutex::new();
	let id = proc_c().futex().insert(futex);
	sysret!(vals, SysErr::Ok.num(), id.into());
}

// TODO: handle timeout option
// TODO: merge common code of futex syscalls together when cleanup kernel code
pub extern "C" fn futex_block(vals: &mut SyscallVals)
{
	let options = FutexOptions::from_bits_truncate(vals.options);
	let id = CapId::from(vals.a1);

	match proc_c().futex().block(id) {
		Ok(_) => sysret!(vals, SysErr::Ok.num()),
		Err(err) => sysret!(vals, err.num()),
	}
}

pub extern "C" fn futex_unblock(vals: &mut SyscallVals)
{
	let options = FutexOptions::from_bits_truncate(vals.options);
	let id = CapId::from(vals.a1);
	let n = vals.a3;

	match proc_c().futex().unblock(id, n) {
		Ok(n) => sysret!(vals, SysErr::Ok.num(), n),
		Err(err) => sysret!(vals, err.num(), 0),
	}
}
