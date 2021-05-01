use spin::Mutex;
use core::ops::Index;
use core::sync::atomic::{AtomicUsize, Ordering};
use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};
use crate::uses::*;
use crate::mem::phys_alloc::zm;
use crate::mem::virt_alloc::{VirtMapper, VirtLayout, VirtLayoutElement, FAllocerType};
use crate::upriv::PrivLevel;
use crate::util::{LinkedList, IMutex};
use super::tlist;
use super::thread::{Thread, ThreadLNode, ThreadState};

static NEXT_PID: AtomicUsize = AtomicUsize::new (0);

#[derive(Debug)]
pub struct ThreadListProcLocal([IMutex<LinkedList<ThreadLNode>>; 2]);

impl ThreadListProcLocal
{
	const fn new () -> Self
	{
		ThreadListProcLocal([
			IMutex::new (LinkedList::new ()),
			IMutex::new (LinkedList::new ()),
		])
	}

	pub fn get (&self, state: ThreadState) -> Option<&IMutex<LinkedList<ThreadLNode>>>
	{
		match state
		{
			ThreadState::Join(_) => Some(&self.0[0]),
			ThreadState::FutexBlock(_) => Some(&self.0[1]),
			_ => None
		}
	}
}

impl Index<ThreadState> for ThreadListProcLocal
{
	type Output = IMutex<LinkedList<ThreadLNode>>;

	fn index (&self, state: ThreadState) -> &Self::Output
	{
		self.get (state).expect ("attempted to index ThreadState with invalid state")
	}
}

#[derive(Debug)]
pub struct Process
{
	pid: usize,
	name: String,
	self_ref: Weak<Self>,

	uid: PrivLevel,

	next_tid: AtomicUsize,
	threads: Mutex<BTreeMap<usize, Arc<Thread>>>,

	pub tlproc: ThreadListProcLocal,

	pub addr_space: VirtMapper<FAllocerType>,
}

impl Process
{
	pub fn new (uid: PrivLevel, name: String) -> Arc<Self>
	{
		Arc::new_cyclic (|weak| Self {
			pid: NEXT_PID.fetch_add (1, Ordering::Relaxed),
			name,
			self_ref: weak.clone (),
			uid,
			next_tid: AtomicUsize::new (0),
			threads: Mutex::new (BTreeMap::new ()),
			tlproc: ThreadListProcLocal::new (),
			addr_space: VirtMapper::new (&zm),
		})
	}

	pub fn from_elf (elf_data: &[u8], uid: PrivLevel, name: String) -> Arc<Self>
	{
		unimplemented! ();
	}

	pub fn pid (&self) -> usize
	{
		self.pid
	}

	pub fn uid (&self) -> PrivLevel
	{
		self.uid
	}

	pub fn next_tid (&self) -> usize
	{
		self.next_tid.fetch_add (1, Ordering::Relaxed)
	}

	// returns false if thread with tid is already inserted or tid was not gotten by next tid func
	pub fn insert_thread (&self, thread: Arc<Thread>) -> bool
	{
		if thread.tid () >= self.next_tid.load (Ordering::Relaxed)
		{
			return false;
		}

		let mut threads = self.threads.lock ();
		match threads.get (&thread.tid ())
		{
			Some(_) => false,
			None => {
				threads.insert (thread.tid (), thread);
				true
			}
		}
	}

	pub fn remove_thread (&self, tid: usize) -> Option<Arc<Thread>>
	{
		self.threads.lock ().remove (&tid)
	}

	// returns tid in ok
	// locks thread list
	pub fn new_thread (&self, thread_func: fn() -> ()) -> Result<usize, Err>
	{
		let tid = self.next_tid ();
		let thread = Thread::new (self.self_ref.clone (), tid, format! ("{}-thread{}", self.name, tid), thread_func as usize)?;
		let tweak = Arc::downgrade (&thread);
		if self.insert_thread (thread)
		{
			let tnode = ThreadLNode::new (tweak);
			tnode.set_state (ThreadState::Ready);
			tlist.lock ()[ThreadState::Ready].push (tnode);
			Ok(tid)
		}
		else
		{
			Err(Err::new ("could not insert thread into process thread list"))
		}
	}
}

impl Drop for Process
{
	fn drop (&mut self)
	{
		// Need to drop all the threads first, because they asssume process is always alive if they are alive
		// TODO: make this faster
		let mut threads = self.threads.lock ();
		while let Some(_) = threads.pop_first () {}

		for mutex in self.tlproc.0.iter ()
		{
			for tpointer in mutex.lock ().iter_mut ()
			{
				unsafe { tpointer.dealloc (); }
			}
		}
	}
}
