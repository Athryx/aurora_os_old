use spin::Mutex;
use ptr::NonNull;
use core::time::Duration;
use core::fmt;
use alloc::collections::BTreeMap;
use alloc::alloc::{Global, Allocator, Layout};
use alloc::sync::{Arc, Weak};
use crate::uses::*;
use crate::mem::phys_alloc::{zm, Allocation};
use crate::mem::virt_alloc::{VirtMapper, VirtLayout, VirtLayoutElement, FAllocerType, PageTableFlags};
use crate::mem::{PAGE_SIZE, VirtRange};
use crate::upriv::PrivLevel;
use crate::util::{ListNode, IMutex, IMutexGuard};
use crate::time::timer;
use super::process::{Process, ThreadListProcLocal};
use super::{Registers, ThreadList, int_sched, tlist};

const USER_REGS: Registers = Registers::new (0x3202, 0x23, 0x1b);
const IOPRIV_REGS: Registers = Registers::new (0x202, 0x23, 0x1b);
const SUPERUSER_REGS: Registers = Registers::new (0x202, 0x23, 0x1b);
const KERNEL_REGS: Registers = Registers::new (0x202, 0x08, 0x10);

const DEFAULT_STACK_SIZE: usize = PAGE_SIZE * 32;
const DEFAULT_KSTACK_SIZE: usize = PAGE_SIZE * 16;

// TODO: implement support for growing stack
#[derive(Debug)]
enum Stack
{
	User(VirtRange),
	Kernel(Allocation),
	KernelNoAlloc(VirtRange),
}

impl Stack
{
	// size in bytes
	fn user_new (size: usize, mapper: &VirtMapper<FAllocerType>) -> Result<Self, Err>
	{
		let allocation = zm.alloc (size)?;

		let mut elem_vec = Vec::new ();
		elem_vec.push (VirtLayoutElement::Empty(PAGE_SIZE));
		elem_vec.push (VirtLayoutElement::AllocedMem(allocation));
		let vlayout = VirtLayout::new (elem_vec);

		let flags = PageTableFlags::WRITABLE | PageTableFlags::NO_EXEC;
		let vrange = unsafe { mapper.map (vlayout, flags)? };

		Ok(Self::User(vrange))
	}

	fn kernel_new (size: usize) -> Result<Self, Err>
	{
		let allocation = zm.alloc (size)?;

		Ok(Self::Kernel(allocation))
	}

	fn no_alloc_new (range: VirtRange) -> Self
	{
		Self::KernelNoAlloc(range)
	}

	unsafe fn dealloc (&self, mapper: &VirtMapper<FAllocerType>)
	{
		match self
		{
			Self::User(vrange) => mapper.unmap (*vrange).unwrap ().dealloc (),
			Self::Kernel(allocation) => zm.dealloc (*allocation),
			_ => ()
		}
	}

	fn bottom (&self) -> usize
	{
		match self
		{
			Self::User(vrange) => vrange.addr ().as_u64 () as usize + PAGE_SIZE,
			Self::Kernel(allocation) => allocation.as_usize (),
			Self::KernelNoAlloc(vrange) => vrange.addr ().as_u64 () as usize,
		}
	}

	fn top (&self) -> usize
	{
		self.bottom () + self.size ()
	}

	fn size (&self) -> usize
	{
		match self
		{
			Self::User(vrange) => vrange.size () - PAGE_SIZE,
			Self::Kernel(allocation) => allocation.len (),
			Self::KernelNoAlloc(vrange) => vrange.size (),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ThreadState
{
	Running,
	Ready,
	Destroy,
	// nsecs to sleep
	Sleep(u64),
	// tid to join with
	Join(usize),
	// virtual address currently waiting on
	FutexBlock(usize),
}

impl ThreadState
{
	// is the data structure for storing this thread local to the process
	pub fn is_proc_local (&self) -> bool
	{
		match self
		{
			Self::Join(_) => true,
			Self::FutexBlock(_) => true,
			_ => false,
		}
	}

	pub fn sleep_nsec (&self) -> Option<u64>
	{
		match self
		{
			Self::Sleep(nsec) => Some(*nsec),
			_ => None,
		}
	}

	pub fn join_tid (&self) -> Option<usize>
	{
		match self
		{
			Self::Join(tid) => Some(*tid),
			_ => None,
		}
	}

	pub fn futex_wait_addr (&self) -> Option<usize>
	{
		match self
		{
			Self::FutexBlock(addr) => Some(*addr),
			_ => None,
		}
	}
}

#[derive(Debug)]
pub struct Thread
{
	tid: usize,
	name: String,

	process: Weak<Process>,
	pub regs: IMutex<Registers>,
	stack: Stack,
	kstack: Option<Stack>,

	run_time: usize,
}

impl Thread
{
	pub fn new (process: Weak<Process>, tid: usize, name: String, rip: usize) -> Result<Arc<Self>, Err>
	{
		Self::new_stack_size (process, tid, name, rip, DEFAULT_STACK_SIZE, DEFAULT_KSTACK_SIZE)
	}

	// kstack_size only applies for ring 3 processes, for ring 0 stack_size is used as the stack size, but the stack is still a kernel stack
	// does not put thread inside scheduler queue
	pub fn new_stack_size (process: Weak<Process>, tid: usize, name: String, rip: usize, stack_size: usize, kstack_size: usize) -> Result<Arc<Self>, Err>
	{
		let proc = &process.upgrade ().expect ("somehow Thread::new ran in a destroyed process");
		let mapper = &proc.addr_space;
		let uid = proc.uid ();

		let mut regs = match uid
		{
			PrivLevel::Kernel => KERNEL_REGS,
			PrivLevel::SuperUser => SUPERUSER_REGS,
			PrivLevel::IOPriv => IOPRIV_REGS,
			PrivLevel::User(_) => USER_REGS,
		};

		let stack = match uid
		{
			PrivLevel::Kernel => Stack::kernel_new (stack_size)?,
			_ => Stack::user_new (stack_size, mapper)?,
		};

		let kstack = match uid
		{
			PrivLevel::Kernel => None,
			_ => Some(Stack::kernel_new (kstack_size)?),
		};

		regs.rip = rip;
		regs.rsp = stack.top () - 16;

		Ok(Arc::new (Thread {
			tid,
			name,
			process,
			regs: IMutex::new (regs),
			stack,
			kstack,
			run_time: 0,
		}))
	}

	// only used for kernel idle thread
	// uid is assumed kernel
	pub fn from_stack (process: Weak<Process>, tid: usize, name: String, rip: usize, range: VirtRange) -> Result<Arc<Self>, Err>
	{
		// TODO: this hass to be ensured in smp
		let _proc = &process.upgrade ().expect ("somehow Thread::new ran in a destroyed process");

		let mut regs = KERNEL_REGS;

		let stack = Stack::no_alloc_new (range);

		regs.rip = rip;
		regs.rsp = stack.top () - 16;

		Ok(Arc::new (Thread {
			tid,
			name,
			process,
			regs: IMutex::new (regs),
			stack,
			kstack: None,
			run_time: 0,
		}))
	}

	pub fn process (&self) -> Arc<Process>
	{
		self.process.upgrade ().expect ("process should be alive")
	}

	pub fn tid (&self) -> usize
	{
		self.tid
	}
}

impl Drop for Thread
{
	fn drop (&mut self)
	{
		let mapper = &self.process ().addr_space;
		unsafe
		{
			self.stack.dealloc (mapper);
			self.kstack.as_ref ().map (|stack| stack.dealloc (mapper));
		}
	}
}

unsafe impl Send for Thread {}
// This isn't true on its own, but there will be checks in the scheduler
unsafe impl Sync for Thread {}

// TODO: should probably merge into one type with Thread
pub struct ThreadLNode
{
	pub thread: Weak<Thread>,
	state: ThreadState,
	prev: *mut Self,
	next: *mut Self,
}

impl ThreadLNode
{
	const LAYOUT: Layout = unsafe { Layout::from_size_align_unchecked (size_of::<Self> (), core::mem::align_of::<Self> ()) };

	pub fn new<'a> (thread: Weak<Thread>) -> &'a mut Self
	{
		let mem = Global.allocate (Self::LAYOUT).expect ("out of memory for ThreadLNode");
		let ptr = mem.as_ptr () as *mut Self;
		let out = ThreadLNode {
			thread,
			state: ThreadState::Ready,
			prev: null_mut (),
			next: null_mut (),
		};

		unsafe
		{
			ptr::write (ptr, out);
			ptr.as_mut ().unwrap ()
		}
	}

	pub fn is_alive (&self) -> bool
	{
		self.thread.strong_count () != 0
	}

	pub fn thread (&self) -> Option<Arc<Thread>>
	{
		self.thread.upgrade ()
	}

	pub fn state (&self) -> ThreadState
	{
		self.state
	}

	pub fn set_state (&mut self, state: ThreadState)
	{
		self.state = state;
	}

	// returns false if failed to remove
	pub fn remove_from_current (&mut self, gtlist: Option<&mut ThreadList>, proctlist: Option<&mut ThreadListProcLocal>) -> bool
	{
		if !self.state.is_proc_local ()
		{
			match gtlist
			{
				Some(list) => {
					list[self.state].remove_node (self);
					true
				},
				None => false,
			}
		}
		else if self.is_alive ()
		{
			match proctlist
			{
				Some(list) => {
					list[self.state].remove_node (self);
					true
				},
				None => false,
			}
		}
		else
		{
			false
		}
	}

	// returns false if failed to insert into list
	// inserts into currnet state
	pub fn insert_into (&mut self, gtlist: Option<&mut ThreadList>, proctlist: Option<&mut ThreadListProcLocal>) -> bool
	{
		if !self.state.is_proc_local ()
		{
			match gtlist
			{
				Some(list) => {
					list[self.state].push (self);
					true
				},
				None => false,
			}
		}
		else if self.is_alive ()
		{
			match proctlist
			{
				Some(list) => {
					list[self.state].push (self);
					true
				},
				None => false,
			}
		}
		else
		{
			false
		}
	}

	// moves ThreadLNode from old thread state data structure to specified new thread state data structure and return true
	// will set state variable accordingly
	// if the thread has already been destroyed via process exiting, this will return false
	pub fn move_to (&mut self, state: ThreadState, mut gtlist: Option<&mut ThreadList>, mut proctlist: Option<&mut ThreadListProcLocal>) -> bool
	{
		if !self.remove_from_current (gtlist.as_mut ().map (|t| &mut **t), proctlist.as_mut ().map (|t| &mut **t))
		{
			return false;
		}
		self.state = state;
		self.insert_into (gtlist, proctlist)
	}

	pub fn block (&mut self, state: ThreadState)
	{
		// do nothing if new state is stil running
		if let ThreadState::Running = state
		{
			return;
		}

		self.state = state;

		int_sched ();
	}

	pub fn sleep (&mut self, duration: Duration)
	{
		self.sleep_until (timer.nsec () + duration.as_nanos () as u64);
	}

	pub fn sleep_until (&mut self, nsec: u64)
	{
		self.block (ThreadState::Sleep(nsec));
	}

	// TODO: figure out if it is safe to drop data pointed to by self
	// will also put threas that are waiting on this thread in ready queue,
	// but only if process is still alive and thread_list is not None
	pub unsafe fn dealloc (&mut self, thread_list: Option<&mut ThreadList>)
	{
		// FIXME: smp reace condition if process is freed after initial thread () call
		self.thread ().map (|thread| {
			thread.process ().remove_thread (thread.tid, thread_list).expect ("thread should have been in process");
		});

		let Self {
			thread,
			state: _,
			prev: _,
			next: _,
		} = self;

		ptr::drop_in_place (thread as *mut Weak<Thread>);
		let ptr = NonNull::new (self as *mut Self).unwrap ().cast ();
		Global.deallocate (ptr, Self::LAYOUT);
	}
}

impl fmt::Debug for ThreadLNode
{
	fn fmt (&self, f: &mut fmt::Formatter<'_>) -> fmt::Result
	{
		f.debug_struct ("ThreadLNode")
			.field ("thread", &self.thread ())
			.field ("state", &self.state)
			.field ("prev", &self.prev)
			.field ("next", &self.next)
			.finish ()
	}
}

crate::impl_list_node! (ThreadLNode, prev, next);
