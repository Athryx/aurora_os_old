use spin::Mutex;
use core::ops::{Index, IndexMut};
use core::sync::atomic::{AtomicUsize, Ordering};
use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};
use crate::uses::*;
use crate::mem::phys_alloc::zm;
use crate::mem::virt_alloc::{VirtMapper, VirtLayout, VirtLayoutElement, PageTableFlags, FAllocerType};
use crate::upriv::PrivLevel;
use crate::util::{LinkedList, IMutex};
use super::{ThreadList, tlist, proc_list};
use super::elf::{ElfParser, Section};
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
	// NOTE: must insert into process list before making a thread
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

	// NOTE: this doesn't quite adhere to elf format I think
	// ignores align field, does not enforce that p_vaddr == P_offset % p_align
	// different segments also must not have any overlapping page frames
	pub fn from_elf (elf_data: &[u8], uid: PrivLevel, name: String) -> Result<Arc<Self>, Err>
	{
		let process = Process::new (uid, name);

		let elf = ElfParser::new (elf_data)?;
		let sections = elf.program_headers ();

		let priv_flag = if uid.as_cpu_priv ().is_ring3 ()
		{
			PageTableFlags::USER
		}
		else
		{
			PageTableFlags::NONE
		};

		for section in sections.iter ()
		{
			let mut flags = priv_flag;

			let sf = section.flags;
			if sf.writable ()
			{
				flags |= PageTableFlags::WRITABLE;
			}
			if !sf.executable ()
			{
				flags |= PageTableFlags::NO_EXEC;
			}

			// allocate section backing memory
			// guarenteed to be aligned
			let vrange = section.virt_range;
			let mut mem  = zm.allocz (vrange.size ())
				.ok_or_else (|| Err::new ("not enough memory to load executable"))?;

			// copy section data over to memory
			let memslice = unsafe
			{
				// mem is guarenteed to have enough space
				core::slice::from_raw_parts_mut (mem.as_mut_ptr::<u8> ().add (section.data_offset), section.data.len ())
			};
			memslice.copy_from_slice (section.data);

			// construct virtaddr layout
			let v_elem = VirtLayoutElement::AllocedMem (mem);
			let vec = vec![v_elem];

			let layout = VirtLayout::new (vec);

			unsafe
			{
				process.addr_space.map_at (layout, vrange, flags)?;
			}
		}

		// in order to avoid a race condition
		// FIXME: this is kind of messy that we have to do this
		let mut plist = proc_list.lock ();
		let pid = process.pid ();
		plist.insert (pid, process.clone ());

		process.new_thread (elf.entry_point (), None).map_err (|err| {
			plist.remove (&pid);
			err
		})?;

		Ok(process)
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
	// NOTE: acquires the tlproc lock and tlist lock
	// TODO: make process die after last thread removed
	pub fn remove_thread (&self, tid: usize) -> Option<Arc<Thread>>
	{
		let out = self.threads.lock ().remove (&tid)?;
		let mut thread_list = tlist.lock ();
		let mut list = self.tlproc.lock ();

		// FIXME: ugly
		for tpointer in unsafe { unbound_mut (&mut list[ThreadState::Join(0)]).iter_mut () }
		{
			let join_tid = tpointer.state ().join_tid ().unwrap ();

			if tid == join_tid
			{
				tpointer.move_to (ThreadState::Ready, Some(&mut thread_list), Some(&mut list));
			}
		}

		drop (thread_list);

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
		while threads.pop_first ().is_some () {}

		loop
		{
			let mut tlproc = self.tlproc.lock ();
			let tpointer = match tlproc[ThreadState::Join(0)].pop_front ()
			{
				Some(thread) => thread,
				None => break,
			};

			// TODO: this is probably slow
			// avoid race condition with dealloc
			drop (tlproc);

			unsafe
			{
				tpointer.dealloc ();
			}
		}
		loop
		{
			let mut tlproc = self.tlproc.lock ();
			let tpointer = match tlproc[ThreadState::FutexBlock(0)].pop_front ()
			{
				Some(thread) => thread,
				None => break,
			};

			// TODO: this is probably slow
			// avoid race condition with dealloc
			drop (tlproc);

			unsafe
			{
				tpointer.dealloc ();
			}
		}
	}
}
