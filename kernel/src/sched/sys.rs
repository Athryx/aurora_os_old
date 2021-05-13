use crate::syscall::SyscallVals;
use super::*;

pub extern "C" fn thread_new (_: &SyscallVals, _: u32, thread_func: usize) -> usize
{
	proc_c ().new_thread (unsafe { core::mem::transmute (thread_func) }, None).unwrap_or (usize::MAX)
}

pub extern "C" fn thread_block (_: &SyscallVals, _: u32, reason: usize, arg: usize) -> usize
{
	match reason
	{
		0 => thread_c ().block (ThreadState::Running),
		1 => thread_c ().block (ThreadState::Destroy),
		2 => thread_c ().block (ThreadState::Sleep(arg as u64)),
		3 => thread_c ().block (ThreadState::Join (arg)),
		_ => (),
	}
	0
}
