use core::time::Duration;
use core::fmt;
use core::sync::atomic::{AtomicPtr, AtomicU64, Ordering};
use core::mem::transmute;
use alloc::collections::BTreeMap;
use alloc::alloc::{Allocator, Global, Layout};
use alloc::sync::{Arc, Weak};

use spin::Mutex;
use ptr::NonNull;
use sys_consts::SysErr;

use crate::uses::*;
use crate::mem::phys_alloc::{zm, Allocation};
use crate::mem::virt_alloc::{
	AllocType, FAllocerType, PageMappingFlags, VirtLayout, VirtLayoutElement, VirtMapper,
};
use crate::mem::{VirtRange, PAGE_SIZE};
use crate::upriv::PrivLevel;
use crate::util::{
	Futex, FutexGuard, IMutex, IMutexGuard, ListNode, MemOwner, UniqueMut, UniquePtr, UniqueRef,
};
use crate::time::timer;
use super::process::Process;
use super::{int_sched, thread_c, tlist, ConnPid, KFutex, MsgArgs, Registers, ThreadList};

// TODO: implement support for growing stack
#[derive(Debug)]
pub enum Stack
{
	User(VirtRange),
	Kernel(Allocation),
	KernelNoAlloc(VirtRange),
}

impl Stack
{
	pub const DEFAULT_SIZE: usize = PAGE_SIZE * 32;
	pub const DEFAULT_KERNEL_SIZE: usize = PAGE_SIZE * 16;
	// size in bytes
	pub fn user_new(size: usize, mapper: &VirtMapper<FAllocerType>) -> Result<Self, Err>
	{
		let elem_vec = vec![
			VirtLayoutElement::new(PAGE_SIZE, PageMappingFlags::NONE)?,
			VirtLayoutElement::new(
				size,
				PageMappingFlags::READ | PageMappingFlags::WRITE | PageMappingFlags::USER,
			)?,
		];

		let vlayout = VirtLayout::from(elem_vec, AllocType::Protected);

		let vrange = unsafe { mapper.map(vlayout)? };

		Ok(Self::User(vrange))
	}

	// TODO: put guard page in this one
	pub fn kernel_new(size: usize) -> Result<Self, Err>
	{
		let allocation = zm.alloc(size)?;

		Ok(Self::Kernel(allocation))
	}

	pub fn no_alloc_new(range: VirtRange) -> Self
	{
		Self::KernelNoAlloc(range)
	}

	pub unsafe fn dealloc(&self, mapper: &VirtMapper<FAllocerType>)
	{
		match self {
			Self::User(vrange) => mapper
				.unmap(*vrange, AllocType::Protected)
				.unwrap()
				.dealloc(),
			Self::Kernel(allocation) => zm.dealloc(*allocation),
			_ => (),
		}
	}

	pub fn bottom(&self) -> usize
	{
		match self {
			Self::User(vrange) => vrange.addr().as_u64() as usize + PAGE_SIZE,
			Self::Kernel(allocation) => allocation.as_usize(),
			Self::KernelNoAlloc(vrange) => vrange.addr().as_u64() as usize,
		}
	}

	pub fn top(&self) -> usize
	{
		self.bottom() + self.size()
	}

	pub fn size(&self) -> usize
	{
		match self {
			Self::User(vrange) => vrange.size() - PAGE_SIZE,
			Self::Kernel(allocation) => allocation.len(),
			Self::KernelNoAlloc(vrange) => vrange.size(),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ThreadState
{
	Running,
	Ready,
	Destroy,
	// waiting for thread to call int_sched
	Waiting(Tuid),
	// nsecs to sleep
	Sleep(u64),
	// tid to join with
	Join(Tuid),
	// virtual address currently waiting on
	FutexBlock(*const KFutex),
	// connection cpid we are waiting for a reply from
	Listening(ConnPid),
}

impl ThreadState
{
	// called by the scheduler when whole scheduler is locked so additional atomic steps can be done
	pub fn atomic_process(&self)
	{
		match self {
			Self::FutexBlock(id) => unsafe { id.as_ref().unwrap().force_unlock() },
			_ => (),
		}
	}
}

#[derive(Debug)]
pub struct ConnSaveState
{
	regs: Registers,
	stack: Stack,
}

impl ConnSaveState
{
	pub fn new(regs: Registers, stack: Stack) -> Self
	{
		ConnSaveState {
			regs,
			stack,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tuid
{
	pid: usize,
	tid: usize,
}

impl Tuid
{
	pub const fn new(pid: usize, tid: usize) -> Tuid
	{
		Tuid {
			pid,
			tid,
		}
	}

	pub fn pid(&self) -> usize
	{
		self.pid
	}

	pub fn tid(&self) -> usize
	{
		self.tid
	}
}

impl Default for Tuid
{
	fn default() -> Self
	{
		Self::new(0, 0)
	}
}

// TODO: should probably merge into one type with Thread
// all information relevent to scheduler is in here (except regs, but thos will probably be moved to here)
pub struct Thread
{
	process: Weak<Process>,
	tuid: Tuid,
	name: String,

	// TODO: find a better solution
	//state: AtomicU128,
	state: IMutex<ThreadState>,
	run_time: AtomicU64,

	pub regs: IMutex<Registers>,
	stack: Futex<Stack>,
	kstack: Option<Stack>,

	conn_data: Futex<Vec<ConnSaveState>>,
	msg_recieve_regs: IMutex<Result<Registers, SysErr>>,

	prev: AtomicPtr<Self>,
	next: AtomicPtr<Self>,
}

impl Thread
{
	pub fn new(
		process: Weak<Process>,
		tid: usize,
		name: String,
		regs: Registers,
	) -> Result<MemOwner<Self>, Err>
	{
		Self::new_stack_size(
			process,
			tid,
			name,
			regs,
			Stack::DEFAULT_SIZE,
			Stack::DEFAULT_KERNEL_SIZE,
		)
	}

	// kstack_size only applies for ring 3 processes, for ring 0 stack_size is used as the stack size, but the stack is still a kernel stack
	// does not put thread inside scheduler queue
	pub fn new_stack_size(
		process: Weak<Process>,
		tid: usize,
		name: String,
		mut regs: Registers,
		stack_size: usize,
		kstack_size: usize,
	) -> Result<MemOwner<Self>, Err>
	{
		let proc = &process
			.upgrade()
			.expect("somehow Thread::new ran in a destroyed process");
		let mapper = &proc.addr_space;
		let uid = proc.uid();

		let stack = match uid {
			PrivLevel::Kernel => Stack::kernel_new(stack_size)?,
			_ => Stack::user_new(stack_size, mapper)?,
		};

		let kstack = match uid {
			PrivLevel::Kernel => None,
			_ => {
				let stack = Stack::kernel_new(kstack_size)?;
				regs.call_rsp = stack.top() - 8;
				Some(stack)
			},
		};

		regs.apply_priv(uid);
		regs.apply_stack(&stack);

		Ok(MemOwner::new(Thread {
			process,
			tuid: Tuid::new(proc.pid(), tid),
			name,
			//state: AtomicU128::new (ThreadState::Ready.as_u128 ()),
			state: IMutex::new(ThreadState::Ready),
			run_time: AtomicU64::new(0),
			regs: IMutex::new(regs),
			stack: Futex::new(stack),
			kstack,
			conn_data: Futex::new(Vec::new()),
			msg_recieve_regs: IMutex::new(Err(SysErr::Unknown)),
			prev: AtomicPtr::new(null_mut()),
			next: AtomicPtr::new(null_mut()),
		}))
	}

	// only used for kernel idle thread
	// uid is assumed kernel
	pub fn from_stack(
		process: Weak<Process>,
		tid: usize,
		name: String,
		mut regs: Registers,
		range: VirtRange,
	) -> Result<MemOwner<Self>, Err>
	{
		// TODO: this hass to be ensured in smp
		let proc = &process
			.upgrade()
			.expect("somehow Thread::new ran in a destroyed process");

		let stack = Stack::no_alloc_new(range);

		regs.apply_priv(PrivLevel::Kernel);
		regs.apply_stack(&stack);

		Ok(MemOwner::new(Thread {
			process,
			tuid: Tuid::new(proc.pid(), tid),
			name,
			//state: AtomicU128::new (ThreadState::Ready.as_u128 ()),
			state: IMutex::new(ThreadState::Ready),
			run_time: AtomicU64::new(0),
			regs: IMutex::new(regs),
			stack: Futex::new(stack),
			kstack: None,
			conn_data: Futex::new(Vec::new()),
			msg_recieve_regs: IMutex::new(Err(SysErr::Unknown)),
			prev: AtomicPtr::new(null_mut()),
			next: AtomicPtr::new(null_mut()),
		}))
	}

	// for future compatability, when thread could be dead because of other reasons
	pub fn is_alive(&self) -> bool
	{
		self.proc_alive()
	}

	pub fn proc_alive(&self) -> bool
	{
		self.process.strong_count() != 0
	}

	pub fn process(&self) -> Option<Arc<Process>>
	{
		self.process.upgrade()
	}

	pub fn tuid(&self) -> Tuid
	{
		self.tuid
	}

	pub fn tid(&self) -> usize
	{
		self.tuid.tid()
	}

	pub fn name(&self) -> &str
	{
		&self.name
	}

	pub fn state(&self) -> ThreadState
	{
		//ThreadState::from_u128 (self.state.load ())
		*self.state.lock()
	}

	pub fn set_state(&self, state: ThreadState)
	{
		//self.state.store (state.as_u128 ())
		*self.state.lock() = state;
	}

	pub fn rcv_regs(&self) -> &IMutex<Result<Registers, SysErr>>
	{
		&self.msg_recieve_regs
	}

	pub fn msg_rcv(&self, args: &MsgArgs)
	{
		let mut regs = *self.regs.lock();
		regs.apply_msg_args(args);
		*self.msg_recieve_regs.lock() = Ok(regs);

		if let ThreadState::Listening(_) = self.state() {
			// FIXME: ugly
			let ptr = UniqueRef::new(self);
			let mut thread_list = tlist.lock();
			Thread::move_to(ptr, ThreadState::Ready, &mut thread_list);
		}
	}

	pub fn push_conn_state(&self, args: &MsgArgs) -> Result<(), SysErr>
	{
		let new_stack =
			match Stack::user_new(Stack::DEFAULT_SIZE, &self.process().unwrap().addr_space) {
				Ok(stack) => stack,
				Err(_) => return Err(SysErr::OutOfMem),
			};

		let regs = *self.regs.lock();
		let mut new_regs = regs;
		new_regs.apply_msg_args(args).apply_stack(&new_stack);

		// shouldn't be race condition, because these are all leaf locks
		let mut rcv_regs = self.msg_recieve_regs.lock();
		let mut conn_state = self.conn_data.lock();
		let mut stack = self.stack.lock();

		let old_stack = core::mem::replace(&mut *stack, new_stack);

		let save_state = ConnSaveState::new(regs, old_stack);
		conn_state.push(save_state);

		*rcv_regs = Ok(new_regs);

		Ok(())
	}

	pub fn pop_conn_state(&self) -> Result<(), SysErr>
	{
		let mut rcv_regs = self.msg_recieve_regs.lock();
		let mut conn_state = self.conn_data.lock();
		let mut stack = self.stack.lock();

		let save_state = conn_state.pop().ok_or(SysErr::InvlOp)?;
		*rcv_regs = Ok(save_state.regs);
		let old_stack = core::mem::replace(&mut *stack, save_state.stack);

		drop(rcv_regs);
		drop(conn_state);
		drop(stack);

		unsafe {
			old_stack.dealloc(&self.process().unwrap().addr_space);
		}
		Ok(())
	}

	// returns false if failed to remove
	pub fn remove_from_current<'a, T>(ptr: T, list: &mut ThreadList) -> MemOwner<Thread>
	where
		T: UniquePtr<Self> + 'a,
	{
		list[ptr.state()].remove_node(ptr)
	}

	// returns None if failed to insert into list
	// inserts into current state list
	pub fn insert_into(thread: MemOwner<Self>, list: &mut ThreadList) -> UniqueMut<Thread>
	{
		list[thread.state()].push(thread)
	}

	// moves ThreadLNode from old thread state data structure to specified new thread state data structure and return true
	// will set state variable accordingly
	// if the thread has already been destroyed via process exiting, this will return false
	pub fn move_to<'a, 'b, T>(
		ptr: T,
		state: ThreadState,
		list: &'a mut ThreadList,
	) -> UniqueMut<'a, Thread>
	where
		T: UniquePtr<Self> + Clone + 'b,
	{
		let thread = Self::remove_from_current(ptr, list);
		thread.set_state(state);
		Self::insert_into(thread, list)
	}

	// only call on currently running thread
	pub fn block(&self, state: ThreadState)
	{
		match state {
			// do nothing if new state is stil running
			ThreadState::Running => return,
			_ => tlist.ensure(state),
		}

		self.set_state(state);

		int_sched();
	}

	pub fn sleep(&self, duration: Duration)
	{
		self.sleep_until(timer.nsec() + duration.as_nanos() as u64);
	}

	pub fn sleep_until(&self, nsec: u64)
	{
		self.block(ThreadState::Sleep(nsec));
	}

	pub fn run_time(&self) -> u64
	{
		self.run_time.load(Ordering::Acquire)
	}

	pub fn inc_time(&self, nsec: u64)
	{
		self.run_time.fetch_add(nsec, Ordering::AcqRel);
	}

	// TODO: figure out if it is safe to drop data pointed to by self
	// will also put threas that are waiting on this thread in ready queue,
	// but only if process is still alive and thread_list is not None
	// NOTE: don't call with any IMutexs locked
	// TODO: make safer
	// safety: must call with no other references pointing to self existing
	pub unsafe fn dealloc(this: MemOwner<Self>)
	{
		// FIXME: smp reace condition if process is freed after initial thread () call
		if let Some(process) = this.process() {
			process
				.remove_thread(this.tid())
				.expect("thread should have been in process");
		}

		ptr::drop_in_place(this.ptr_mut());
		this.dealloc();
	}
}

impl Drop for Thread
{
	fn drop(&mut self)
	{
		if let Some(process) = self.process() {
			let mapper = &process.addr_space;
			unsafe {
				self.stack.lock().dealloc(mapper);
				if let Some(stack) = self.kstack.as_ref() {
					stack.dealloc(mapper);
				}
			}
		}
	}
}

impl fmt::Debug for Thread
{
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result
	{
		f.debug_struct("Thread")
			.field("process", &self.process())
			.field("tuid", &self.tuid)
			.field("name", &self.name)
			.field("state", &self.state)
			.field("run_time", &self.run_time)
			.field("regs", &self.regs)
			.field("stack", &self.stack)
			.field("kstack", &self.kstack)
			.field("conn_data", &self.conn_data)
			.field("prev", &self.prev)
			.field("next", &self.next)
			.finish()
	}
}

libutil::impl_list_node!(Thread, prev, next);
