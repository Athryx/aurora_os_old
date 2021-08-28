use crate::uses::*;
use libutil::futex::{Blocker, FutexImpl, FutexImplGuard, RWFutexImpl, RWFutexImplReadGuard, RWFutexImplWriteGuard};
use crate::sched::*;

#[derive(Debug)]
pub struct KBlocker();

impl Blocker for KBlocker
{
	fn block (addr: usize)
	{
		thread_c ().block (ThreadState::FutexBlock(addr));
	}

	fn unblock (addr: usize)
	{
		proc_c ().futex_move (addr, ThreadState::Ready, 1);
	}
}

pub type Futex<T> = FutexImpl<T, KBlocker>;
pub type FutexGuard<'a, T> = FutexImplGuard<'a, T, KBlocker>;

pub type RWFutex<T> = RWFutexImpl<T, KBlocker>;
pub type RWFutexReadGuard<'a, T> = RWFutexImplReadGuard<'a, T, KBlocker>;
pub type RWFutexWriteGuard<'a, T> = RWFutexImplWriteGuard<'a, T, KBlocker>;
