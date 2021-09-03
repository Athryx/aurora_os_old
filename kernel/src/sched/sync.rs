use crate::uses::*;
use spin::{Mutex, MutexGuard};
use core::sync::atomic::{AtomicIsize, AtomicBool, Ordering};
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use super::{thread_c, tlist, ThreadState};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FuidInner
{
	parent_id: usize,
	fid: usize,
}

impl FuidInner
{
	fn new (parent_id: usize, fid: usize) -> Self
	{
		FuidInner {
			parent_id,
			fid,
		}
	}
}

// a struct that uniqeuly identifies a futex for the scheduler
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fuid
{
	Process(FuidInner),
	SMem(FuidInner),
}

impl Fuid
{
	pub fn new_process (parent_id: usize, fid: usize) -> Self
	{
		Self::Process(FuidInner::new (parent_id, fid))
	}

	pub fn new_smem (parent_id: usize, fid: usize) -> Self
	{
		Self::SMem(FuidInner::new (parent_id, fid))
	}
}

impl Default for Fuid
{
	fn default () -> Self
	{
		Self::Process(FuidInner::default ())
	}
}

#[derive(Debug)]
pub struct KFutex
{
	id: Fuid,
	wait_count: AtomicIsize,
	alive: AtomicBool,
	block_lock: Mutex<()>,
}

impl KFutex
{
	fn new (id: Fuid) -> Arc<Self>
	{
		let out = KFutex {
			id,
			wait_count: AtomicIsize::new (0),
			alive: AtomicBool::new (true),
			block_lock: Mutex::new (()),
		};

		let state = ThreadState::FutexBlock(&out as *const _);
		tlist.ensure (state);

		Arc::new (out)
	}

	pub fn fuid (&self) -> Fuid
	{
		self.id
	}

	// safety: only call in atomic_process which is called by scheduler
	pub unsafe fn force_unlock (&self)
	{
		self.block_lock.force_unlock ()
	}

	// returns none if futex destroyed
	fn lock (&self) -> Option<MutexGuard<()>>
	{
		loop
		{
			if !self.alive.load (Ordering::Acquire)
			{
				return None;
			}
			if let Some(lock) = self.block_lock.try_lock ()
			{
				return Some(lock);
			}

			core::hint::spin_loop ();
		}
	}

	/*pub fn wait_count (&self) -> isize
	{
		self.wait_count.get ()
	}

	pub fn inc_wait_count (&self, n: isize)
	{
		let old = self.wait_count.get ();
		self.wait_count.set (old + n);
	}*/

	// returns true if successfully blocked
	fn block (&self) -> bool
	{
		if self.wait_count.fetch_add (1, Ordering::AcqRel) >= 0
		{
			// in order to solve race condition with unblock and drop, we use this lock
			// we lock it here, and call block
			// the scheduler will then lock everything, and will call ThreadState::atomic_process
			// ThreadState::FutexBlock stores a pointer to this data structure, so atomic_process
			// uses this pointer and calls Mutex::force_unlock to unlock the mutex in the scheduler
			// we then forget the lock we hold afterword
			let lock = match self.lock ()
			{
				Some(lock) => lock,
				None => return false,
			};
			thread_c ().block (ThreadState::FutexBlock(self as *const _));
			core::mem::forget (lock);
		}

		true
	}

	// call while btree of futexes is locked, otherwise might cause a race condition
	fn unblock (&self, n: usize) -> usize
	{
		let state = ThreadState::FutexBlock(self as *const _);
		let _lock = self.block_lock.lock ();
		let mut tlock = tlist.lock ();

		// FIXME: bad as cast
		self.wait_count.fetch_sub (n as isize, Ordering::AcqRel);

		tlock.inner_state_move (state, ThreadState::Ready, n)
	}

	// call while btree of futexes is locked, otherwise might cause a race condition
	fn destroy (&self) -> usize
	{
		self.alive.store (false, Ordering::Release);
		let state = ThreadState::FutexBlock(self as *const _);

		let lock = self.block_lock.lock ();
		// forget this lock to stop any other thread from blocking and eventually all spining threads will stop because alive is false
		core::mem::forget (lock);

		let out = tlist.state_move (state, ThreadState::Ready, usize::MAX);
		tlist.dealloc_state (state);
		out
	}
}

#[derive(Debug)]
pub struct FutexMap
{
	process: bool,
	parent_id: usize,
	// TODO: make this prettier
	data: Mutex<BTreeMap<Fuid, Arc<KFutex>>>,
}

impl FutexMap
{
	pub fn new_process (pid: usize) -> Self
	{
		FutexMap {
			process: true,
			parent_id: pid,
			data: Mutex::new (BTreeMap::new ()),
		}
	}

	pub fn new_smem (smid: usize) -> Self
	{
		FutexMap {
			process: false,
			parent_id: smid,
			data: Mutex::new (BTreeMap::new ()),
		}
	}

	fn fuid (&self, id: usize) -> Fuid
	{
		if self.process
		{
			Fuid::new_process (self.parent_id, id)
		}
		else
		{
			Fuid::new_smem (self.parent_id, id)
		}
	}

	fn get_insert (&self, id: usize) -> Arc<KFutex>
	{
		let fuid = self.fuid (id);
		let mut lock = self.data.lock ();
		lock.entry (fuid).or_insert (KFutex::new (fuid)).clone ()
	}

	pub fn block (&self, id: usize)
	{
		while !self.get_insert (id).block () {}
	}

	pub fn unblock (&self, id: usize, n: usize) -> usize
	{
		// don't need to repeatedly retrt because we hold lock the whole time
		let fuid = self.fuid (id);
		let mut lock = self.data.lock ();
		let futex = lock.entry (fuid).or_insert (KFutex::new (fuid));
		futex.unblock (n)
	}

	pub fn destroy (&self, id: usize) -> Result<usize, SysErr>
	{
		let fuid = self.fuid (id);
		let mut lock = self.data.lock ();
		let futex = lock.remove (&fuid).ok_or (SysErr::InvlId)?;
		Ok(futex.destroy ())
	}
}
