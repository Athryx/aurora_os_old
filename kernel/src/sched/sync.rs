use core::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use alloc::collections::BTreeMap;
use alloc::sync::Arc;

use spin::{Mutex, MutexGuard};

use crate::uses::*;
use crate::cap::{CapObject, CapObjectType, CapFlags, Capability, CapId};
use super::{thread_c, tlist, ThreadState};

crate::make_id_type!(Fuid);

#[derive(Debug)]
pub struct KFutex
{
	id: Fuid,
	wait_count: AtomicIsize,
	alive: AtomicBool,
	block_lock: Mutex<()>,
	ref_count: AtomicUsize,
}

impl KFutex
{
	fn new(id: Fuid) -> Arc<Self>
	{
		let out = KFutex {
			id,
			wait_count: AtomicIsize::new(0),
			alive: AtomicBool::new(true),
			block_lock: Mutex::new(()),
			ref_count: AtomicUsize::new(0),
		};

		let state = ThreadState::FutexBlock(&out as *const _);
		tlist.ensure(state);

		Arc::new(out)
	}

	pub fn fuid(&self) -> Fuid
	{
		self.id
	}

	// safety: only call in atomic_process which is called by scheduler
	pub unsafe fn force_unlock(&self)
	{
		self.block_lock.force_unlock()
	}

	// returns none if futex destroyed
	fn lock(&self) -> Option<MutexGuard<()>>
	{
		loop {
			if !self.alive.load(Ordering::Acquire) {
				return None;
			}
			if let Some(lock) = self.block_lock.try_lock() {
				return Some(lock);
			}

			core::hint::spin_loop();
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
	fn block(&self) -> bool
	{
		if self.wait_count.fetch_add(1, Ordering::AcqRel) >= 0 {
			// in order to solve race condition with unblock and drop, we use this lock
			// we lock it here, and call block
			// the scheduler will then lock everything, and will call ThreadState::atomic_process
			// ThreadState::FutexBlock stores a pointer to this data structure, so atomic_process
			// uses this pointer and calls Mutex::force_unlock to unlock the mutex in the scheduler
			// we then forget the lock we hold afterword
			let lock = match self.lock() {
				Some(lock) => lock,
				None => return false,
			};
			thread_c().block(ThreadState::FutexBlock(self as *const _));
			core::mem::forget(lock);
		}

		true
	}

	fn unblock(&self, n: usize) -> usize
	{
		let state = ThreadState::FutexBlock(self as *const _);
		let _lock = self.block_lock.lock();
		let mut tlock = tlist.lock();

		// FIXME: bad as cast
		self.wait_count.fetch_sub(n as isize, Ordering::AcqRel);

		tlock.inner_state_move(state, ThreadState::Ready, n)
	}

	fn destroy(&self) -> usize
	{
		self.alive.store(false, Ordering::Release);
		let state = ThreadState::FutexBlock(self as *const _);

		let lock = self.block_lock.lock();
		// forget this lock to stop any other thread from blocking and eventually all spining threads will stop because alive is false
		core::mem::forget(lock);

		let out = tlist.state_move(state, ThreadState::Ready, usize::MAX);
		tlist.dealloc_state(state);
		out
	}
}

impl CapObject for KFutex {
	fn cap_object_type() -> CapObjectType {
		CapObjectType::Futex
	}

	fn inc_ref(&self) {
		self.ref_count.fetch_add(1, Ordering::Relaxed);
	}

	fn dec_ref(&self) {
		if self.ref_count.fetch_sub(1, Ordering::Relaxed) == 0 {
			self.destroy();
		}
	}
}

#[derive(Debug)]
pub struct FutexMap
{
	process: bool,
	parent_id: usize,
	// TODO: make this prettier
	data: Mutex<BTreeMap<CapId, Capability<KFutex>>>,
}

impl FutexMap
{
	pub fn new_process(pid: usize) -> Self
	{
		FutexMap {
			process: true,
			parent_id: pid,
			data: Mutex::new(BTreeMap::new()),
		}
	}

	pub fn new_smem(smid: usize) -> Self
	{
		FutexMap {
			process: false,
			parent_id: smid,
			data: Mutex::new(BTreeMap::new()),
		}
	}

	pub fn block(&self, cid: CapId) -> Result<(), SysErr>
	{
		// I don't think this is a race condition
		let lock = self.data.lock();
		let cap = lock.get(&cid).ok_or(SysErr::InvlId)?;
		let futex = cap.arc_clone();
		let flags = cap.flags();
		drop(lock);

		if !flags.contains(CapFlags::READ) {
			Err(SysErr::InvlCap)
		} else {
			if futex.block() {
				Ok(())
			} else {
				Err(SysErr::InvlId)
			}
		}
	}

	pub fn unblock(&self, cid: CapId, n: usize) -> Result<usize, SysErr>
	{
		// don't need to repeatedly retrt because we hold lock the whole time
		let mut lock = self.data.lock();
		let futex = lock.get(&cid).ok_or(SysErr::InvlId)?;
		Ok(futex.object().unblock(n))
	}

	pub fn remove(&self, cid: CapId) -> Result<Capability<KFutex>, SysErr>
	{
		let mut lock = self.data.lock();
		Ok(lock.remove(&cid).ok_or(SysErr::InvlId)?)
	}
}
