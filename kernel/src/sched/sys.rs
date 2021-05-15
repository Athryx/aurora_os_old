use crate::syscall::SyscallVals;
use super::*;

pub extern "C" fn thread_new (vals: &mut SyscallVals)
{
	match proc_c ().new_thread (unsafe { core::mem::transmute (thread_func) }, None)
	{
		Ok(tid) => {
			vals.a1 = tid;
			vals.a2 = 0;
		},
		Err(_) => {
			vals.a1 = 0;
			vals.a2 = 1;
		}
	}
}

pub extern "C" fn thread_block (vals: &mut SyscallVals)
{
	match reason
	{
		0 => thread_c ().block (ThreadState::Running),
		1 => thread_c ().block (ThreadState::Destroy),
		2 => thread_c ().block (ThreadState::Sleep(arg as u64)),
		3 => thread_c ().block (ThreadState::Join (arg)),
		_ => (),
	}
}
