use spin::Mutex;
use core::ops::{Index, IndexMut};
use core::sync::atomic::{AtomicUsize, Ordering};
use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};
use crate::uses::*;
use crate::mem::phys_alloc::zm;
use crate::mem::virt_alloc::{VirtMapper, VirtLayout, VirtLayoutElement, FAllocerType};
use crate::upriv::PrivLevel;
use crate::util::{LinkedList, IMutex};
use super::{ThreadList, tlist};
use super::thread::{Thread, ThreadLNode, ThreadState};

static NEXT_PID: AtomicUsize = AtomicUsize::new (0);

#[derive(Debug)]
pub struct ThreadListProcLocal([LinkedList<ThreadLNode>; 2]);

impl ThreadListProcLocal
{
	const fn new () -> Self
	{
		ThreadListProcLocal([
			LinkedList::new (),
			LinkedList::new (),
		])
	}

	pub fn get (&self, state: ThreadState) -> Option<&LinkedList<ThreadLNode>>
	{
		match state
		{
			ThreadState::Join(_) => Some(&self.0[0]),
			ThreadState::FutexBlock(_) => Some(&self.0[1]),
			_ => None
		}
	}

	pub fn get_mut (&mut self, state: ThreadState) -> Option<&mut LinkedList<ThreadLNode>>
	{
		match state
		{
			ThreadState::Join(_) => Some(&mut self.0[0]),
			ThreadState::FutexBlock(_) => Some(&mut self.0[1]),
			_ => None
		}
	}
}

impl Index<ThreadState> for ThreadListProcLocal
{
	type Output = LinkedList<ThreadLNode>;

	fn index (&self, state: ThreadState) -> &Self::Output
	{
		self.get (state).expect ("attempted to index ThreadState with invalid state")
	}
}


impl IndexMut<ThreadState> for ThreadListProcLocal
{
	fn index_mut (&mut self, state: ThreadState) -> &mut Self::Output
	{
		self.get_mut (state).expect ("attempted to index ThreadState with invalid state")
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

	pub tlproc: IMutex<ThreadListProcLocal>,

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
			tlproc: IMutex::new (ThreadListProcLocal::new ()),
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

	// sets any threads waiting on this thread to ready to run if thread_list is Some
	// acquires the tlproc lock
	pub fn remove_thread (&self, tid: usize, thread_list: Option<&mut ThreadList>) -> Option<Arc<Thread>>
	{
		let out = self.threads.lock ().remove (&tid)?;

		if let Some(thread_list) = thread_list
		{
			let mut list = self.tlproc.lock ();

			// FIXME: ugly
			for tpointer in unsafe { unbound_mut (&mut list[ThreadState::Join(0)]).iter_mut () }
			{
				let join_tid = tpointer.state ().join_tid ().unwrap ();

				if tid == join_tid
				{
					tpointer.move_to (ThreadState::Ready, Some(thread_list), Some(&mut list));
				}
			}
		}

		Some(out)
	}

	// returns tid in ok
	// locks thread list
	pub fn new_thread (&self, thread_func: fn() -> (), name: Option<String>) -> Result<usize, Err>
	{
		let tid = self.next_tid ();
		let thread = Thread::new (self.self_ref.clone (), tid, name.unwrap_or_else (|| format! ("{}-thread{}", self.name, tid)), thread_func as usize)?;
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

		let mut tlproc = self.tlproc.lock ();
		for tpointer in tlproc[ThreadState::Join (0)].iter_mut ()
		{
			unsafe { tpointer.dealloc (None); }
		}
		for tpointer in tlproc[ThreadState::FutexBlock (0)].iter_mut ()
		{
			unsafe { tpointer.dealloc (None); }
		}
	}
}
