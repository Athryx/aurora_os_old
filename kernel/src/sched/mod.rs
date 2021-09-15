use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use core::time::Duration;
use core::ops::{Index, IndexMut};
use core::cell::Cell;
use core::ptr::NonNull;
use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};
use alloc::alloc::{Allocator, Global, Layout};

use spin::Mutex;
pub use process::{Pid, Process, SpawnMapFlags, SpawnStartState};
pub use thread::{Stack, Tid, ThreadRef, Thread, ThreadState, Tuid};
pub use sync::{Fuid, FutexMap, KFutex};

use crate::uses::*;
use crate::int::idt::{Handler, INT_SCHED, IRQ_TIMER};
use crate::util::{
	AvlTree, IMutex, IMutexGuard, LinkedList, MemOwner, TreeNode, UniqueMut, UniquePtr, UniqueRef,
};
use crate::arch::x64::{cli, rdmsr, wrmsr, EFER_EXEC_DISABLE, EFER_MSR};
use crate::time::timer;
use crate::upriv::PrivLevel;
use crate::consts::INIT_STACK;
use crate::gdt::tss;
use crate::ipc::Ipcid;

// TODO: clean up code, it is kind of ugly
// use new interrupt disabling machanism

// TODO: the scheduler uses a bad choice of data structures, i'll implement better ones later
// running list should be not list and cpu local
// ready list should be changed once I decide what scheduling algorithm to use
// sleeping threads should be min heap or min tree
// futex list should be hash map or binary tree of linkedlists
// join should probably have a list in each thread that says all the threads that are waiting on them

// FIXME: there are a few possible race conditions with smp and (planned) process exit syscall

// FIXME: there is a current race condition that occurs when using proc_c () function

mod elf;
mod process;
mod sync;
pub mod sys;
mod thread;

pub static tlist: ThreadListGuard = ThreadListGuard::new();
static proc_list: Mutex<BTreeMap<Pid, Arc<Process>>> = Mutex::new(BTreeMap::new());

// amount of time that elapses before we will switch to a new thread in nanoseconds
// current value is 100 milliseconds
const SCHED_TIME: u64 = 100000000;
static last_switch_nsec: AtomicU64 = AtomicU64::new(0);

// TODO: make this cpu local data
// FIXME: this is a bad way to return registers, and won't be safe with smp
pub static mut out_regs: Registers = Registers::zero();

// These interrupt handlers, and all other timer interrupt handlers must not:
// lock any reguler mutexes or spinlocks (only IMutex)
// this means no memory allocation
fn time_handler(regs: &Registers, _: u64) -> Option<&Registers>
{
	lock();

	let mut out = None;

	let nsec = timer.nsec_no_latch();

	let mut thread_list = tlist.lock();
	let time_list = &mut thread_list[ThreadState::Sleep(0)];

	// FIXME: ugly
	for tpointer in unsafe { unbound_mut(time_list).iter() } {
		let sleep_nsec = match tpointer.state() {
			ThreadState::Sleep(nsec) => nsec,
			_ => panic!("thread in sleep queue but state is not sleeping"),
		};

		if nsec >= sleep_nsec {
			Thread::move_to(tpointer, ThreadState::Ready, &mut thread_list);
		}
	}

	// release mutex
	drop(thread_list);

	if nsec - last_switch_nsec.load(Ordering::Relaxed) > SCHED_TIME {
		out = schedule(regs, nsec);
	}

	out
}

// FIXME: this could potentially have a thread switch occur before lock
// when this happens, switching back to this thread will cause it to immediataly give up control again
// this is probably undesirable
fn int_handler(regs: &Registers, _: u64) -> Option<&Registers>
{
	lock();

	let mut thread_list = tlist.lock();
	let tuid = thread_list[ThreadState::Running].g(0).tuid();
	if let Some(wait_list) = thread_list.get_mut(ThreadState::Waiting(tuid)) {
		// FIXME: ugly
		for thread in unsafe { unbound_mut(wait_list).iter() } {
			Thread::move_to(thread, ThreadState::Ready, &mut thread_list);
		}
	}

	drop(thread_list);

	schedule(regs, timer.nsec())
}

fn int_sched()
{
	unsafe {
		asm!("int 128");
	}
}

// schedule takes in current registers, and picks new thread to switch to
// if it doesn't, schedule returns None and does nothing with regs
// if it does, schedule sets the old thread's registers to be regs, switches address
// space if necessary, and returns the new thread's registers
// schedule will disable interrupts if necessary
fn schedule(_regs: &Registers, nsec_current: u64) -> Option<&Registers>
{
	let mut thread_list = tlist.lock();

	let tpointer = loop {
		match thread_list[ThreadState::Ready].pop_front() {
			Some(t) => {
				if !t.is_alive() {
					t.set_state(ThreadState::Destroy);
					Thread::insert_into(t, &mut thread_list);
				} else {
					break t;
				}
			},
			None => return None,
		}
	};

	let old_thread = thread_list[ThreadState::Running]
		.pop()
		.expect("no currently running thread");

	let nsec_last = last_switch_nsec.swap(nsec_current, Ordering::SeqCst);
	if nsec_current >= nsec_last {
		old_thread.inc_time(nsec_current - nsec_last);
	} else {
		rprintln!("WARNING: pit returned time value less than previous time value");
	}

	// FIXME: smp race condition
	let old_process = old_thread.process().unwrap();

	let old_state = old_thread.state();
	old_state.atomic_process();
	if let ThreadState::Running = old_state {
		old_thread.set_state(ThreadState::Ready);
	}

	// if process was dropped, move to destroy list
	Thread::insert_into(old_thread, &mut thread_list);

	// TODO: add premptive multithreading here
	tpointer.set_state(ThreadState::Running);
	let tpointer = Thread::insert_into(tpointer, &mut thread_list);

	//rprintln! ("switching to:\n{:#x?}", *tpointer);

	// FIXME: smp race condition
	let new_process = tpointer.process().unwrap();

	if old_process.pid() != new_process.pid() {
		unsafe {
			new_process.addr_space.load();
		}
	}

	unsafe {
		// FIXME: ugly
		// safe to do if the scheduler is locked until returning from interrupt handler, since the thread can't be freed
		out_regs = *tpointer.regs.lock();
		Some(&out_regs)
	}
}

// TODO: when smp addded, change these
fn lock()
{
	cli();
}

fn thread_cleaner()
{
	// need to use an intermediate list, because we need to hold the lock to the thread list the whole time,
	// but deallocing a thread requires releasing the lock
	let mut list = LinkedList::new();

	loop {
		let mut thread_list = tlist.lock();
		let dlist = &mut thread_list[ThreadState::Destroy];
		for tpointer in unsafe { unbound_mut(dlist).iter() } {
			if tpointer.ref_count() == 0 {
				let t = dlist.remove_node(tpointer);
				list.push(t);
			}
		}

		drop(thread_list);

		// TODO: probably a good idea to put this logic in separate function
		// FIXME: there might be a race condition here (bochs freezes, but not qemu)
		loop {
			let tcell = match list.pop_front() {
				Some(thread) => thread,
				None => break,
			};

			rprintln!("Deallocing thread pointer:\n{:#x?}", tcell);

			unsafe {
				tcell.dealloc();
			}
		}
		sleep(Duration::new(1, 0));
	}
}

// FIXME: find way to dealloc these when unneeded
#[derive(Debug)]
pub struct TLTreeNode<T: Default + Copy>
{
	id: Cell<T>,
	list: LinkedList<Thread>,

	bf: Cell<i8>,
	parent: Cell<*const Self>,
	left: Cell<*const Self>,
	right: Cell<*const Self>,
}

impl<T: Default + Copy> TLTreeNode<T>
{
	pub fn new() -> MemOwner<Self>
	{
		MemOwner::new(TLTreeNode {
			id: Cell::new(T::default()),
			list: LinkedList::new(),
			bf: Cell::new(0),
			parent: Cell::new(null()),
			left: Cell::new(null()),
			right: Cell::new(null()),
		})
	}

	// Safety: MemOwner must point to a valid FutexTreeNode
	pub unsafe fn dealloc(this: MemOwner<Self>)
	{
		this.dealloc();
	}
}

unsafe impl<T: Default + Copy> Send for TLTreeNode<T> {}

libutil::impl_tree_node!(Tuid, TLTreeNode<Tuid>, parent, left, right, id, bf);
libutil::impl_tree_node!(Ipcid, TLTreeNode<Ipcid>, parent, left, right, id, bf);
libutil::impl_tree_node!(Fuid, TLTreeNode<Fuid>, parent, left, right, id, bf);

#[derive(Debug)]
pub struct ThreadList
{
	running: LinkedList<Thread>,
	ready: LinkedList<Thread>,
	destroy: LinkedList<Thread>,
	sleep: LinkedList<Thread>,
	join: AvlTree<Tuid, TLTreeNode<Tuid>>,
	wait: AvlTree<Tuid, TLTreeNode<Tuid>>,
	conn_wait: AvlTree<Ipcid, TLTreeNode<Ipcid>>,
	futex: AvlTree<Fuid, TLTreeNode<Fuid>>,
}

impl ThreadList
{
	const fn new() -> Self
	{
		ThreadList {
			running: LinkedList::new(),
			ready: LinkedList::new(),
			destroy: LinkedList::new(),
			sleep: LinkedList::new(),
			join: AvlTree::new(),
			wait: AvlTree::new(),
			conn_wait: AvlTree::new(),
			futex: AvlTree::new(),
		}
	}

	fn get(&self, state: ThreadState) -> Option<&LinkedList<Thread>>
	{
		match state {
			ThreadState::Running => Some(&self.running),
			ThreadState::Ready => Some(&self.ready),
			ThreadState::Destroy => Some(&self.destroy),
			ThreadState::Sleep(_) => Some(&self.sleep),
			ThreadState::Join(tuid) => Some(unsafe { unbound(&self.join.get(&tuid)?.list) }),
			ThreadState::Waiting(tuid) => Some(unsafe { unbound(&self.wait.get(&tuid)?.list) }),
			ThreadState::Listening(id) => Some(unsafe { unbound(&self.conn_wait.get(&id)?.list) }),
			ThreadState::FutexBlock(id) => {
				Some(unsafe { unbound(&self.futex.get(&id.as_ref().unwrap().fuid())?.list) })
			},
		}
	}

	fn get_mut(&mut self, state: ThreadState) -> Option<&mut LinkedList<Thread>>
	{
		match state {
			ThreadState::Running => Some(&mut self.running),
			ThreadState::Ready => Some(&mut self.ready),
			ThreadState::Destroy => Some(&mut self.destroy),
			ThreadState::Sleep(_) => Some(&mut self.sleep),
			ThreadState::Join(tuid) => {
				Some(unsafe { unbound_mut(&mut self.join.get_mut(&tuid)?.list) })
			},
			ThreadState::Waiting(tuid) => {
				Some(unsafe { unbound_mut(&mut self.wait.get_mut(&tuid)?.list) })
			},
			ThreadState::Listening(id) => {
				Some(unsafe { unbound_mut(&mut self.conn_wait.get_mut(&id)?.list) })
			},
			ThreadState::FutexBlock(id) => Some(unsafe {
				unbound_mut(&mut self.futex.get_mut(&id.as_ref().unwrap().fuid())?.list)
			}),
		}
	}

	pub fn inner_state_move(
		&mut self,
		old_state: ThreadState,
		new_state: ThreadState,
		count: usize,
	) -> usize
	{
		if let ThreadState::Running = new_state {
			panic!("cannot move thread blocked on futex directly to running thread");
		}

		if let ThreadState::Running = old_state {
			panic!("cannot move running thread");
		}

		if self.get(old_state).is_none() || self.get(new_state).is_none() {
			return 0;
		}

		for i in 0..count {
			match self[old_state].pop_front() {
				Some(tpointer) => {
					tpointer.set_state(new_state);
					Thread::insert_into(tpointer, self);
				},
				None => return i,
			}
		}

		count
	}
}

impl Index<ThreadState> for ThreadList
{
	type Output = LinkedList<Thread>;

	fn index(&self, state: ThreadState) -> &Self::Output
	{
		self.get(state)
			.expect("attempted to index ThreadState with invalid state")
	}
}

impl IndexMut<ThreadState> for ThreadList
{
	fn index_mut(&mut self, state: ThreadState) -> &mut Self::Output
	{
		self.get_mut(state)
			.expect("attempted to index ThreadState with invalid state")
	}
}

pub struct ThreadListGuard(IMutex<ThreadList>);

impl ThreadListGuard
{
	pub const fn new() -> Self
	{
		ThreadListGuard(IMutex::new(ThreadList::new()))
	}

	pub fn lock(&self) -> IMutexGuard<ThreadList>
	{
		self.0.lock()
	}

	pub fn state_move(&self, old_state: ThreadState, new_state: ThreadState, count: usize)
		-> usize
	{
		self.ensure(new_state);

		self.lock().inner_state_move(old_state, new_state, count)
	}

	// TODO: have ensure and dealloc_state return locks to guarentee no race conditions
	pub fn ensure(&self, state: ThreadState)
	{
		// TODO: put both these branches into a function
		match state {
			ThreadState::Join(tuid) => {
				if self.lock().join.get(&tuid).is_none() {
					let node = TLTreeNode::new();
					// NOTE: this is non allocing AvlTree, which returns the value it tried to insert if there was already a valus in the tree
					if let Err(val) = self.lock().join.insert(tuid, node) {
						unsafe {
							TLTreeNode::dealloc(val);
						}
					}
				}
			},
			ThreadState::Waiting(tuid) => {
				if self.lock().wait.get(&tuid).is_none() {
					let node = TLTreeNode::new();
					// NOTE: this is non allocing AvlTree, which returns the value it tried to insert if there was already a valus in the tree
					if let Err(val) = self.lock().wait.insert(tuid, node) {
						unsafe {
							TLTreeNode::dealloc(val);
						}
					}
				}
			},
			ThreadState::Listening(cpid) => {
				if self.lock().conn_wait.get(&cpid).is_none() {
					let node = TLTreeNode::new();
					// NOTE: this is non allocing AvlTree, which returns the value it tried to insert if there was already a valus in the tree
					if let Err(val) = self.lock().conn_wait.insert(cpid, node) {
						unsafe {
							TLTreeNode::dealloc(val);
						}
					}
				}
			},
			ThreadState::FutexBlock(id) => {
				let id = unsafe { id.as_ref().unwrap().fuid() };
				if self.lock().futex.get(&id).is_none() {
					let node = TLTreeNode::new();
					// NOTE: this is non allocing AvlTree, which returns the value it tried to insert if there was already a valus in the tree
					if let Err(val) = self.lock().futex.insert(id, node) {
						unsafe {
							TLTreeNode::dealloc(val);
						}
					}
				}
			},
			_ => (),
		}
	}

	pub fn dealloc_state(&self, state: ThreadState)
	{
		match state {
			ThreadState::Join(tuid) => {
				if let Some(node) = self.lock().join.remove(&tuid) {
					assert_eq!(node.list.len(), 0);
					unsafe {
						TLTreeNode::dealloc(node);
					}
				}
			},
			ThreadState::Waiting(tuid) => {
				if let Some(node) = self.lock().wait.remove(&tuid) {
					assert_eq!(node.list.len(), 0);
					unsafe {
						TLTreeNode::dealloc(node);
					}
				}
			},
			ThreadState::Listening(cpid) => {
				if let Some(node) = self.lock().conn_wait.remove(&cpid) {
					assert_eq!(node.list.len(), 0);
					unsafe {
						TLTreeNode::dealloc(node);
					}
				}
			},
			ThreadState::FutexBlock(id) => {
				let id = unsafe { id.as_ref().unwrap().fuid() };
				if let Some(node) = self.lock().futex.remove(&id) {
					assert_eq!(node.list.len(), 0);
					unsafe {
						TLTreeNode::dealloc(node);
					}
				}
			},
			_ => (),
		}
	}
}

// for assembly code to know structure
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Registers
{
	pub rax: usize,
	pub rbx: usize,
	pub rcx: usize,
	pub rdx: usize,
	pub rbp: usize,
	pub rsp: usize,
	pub call_rsp: usize,
	pub call_save_rsp: usize,
	pub rdi: usize,
	pub rsi: usize,
	pub r8: usize,
	pub r9: usize,
	pub r10: usize,
	pub r11: usize,
	pub r12: usize,
	pub r13: usize,
	pub r14: usize,
	pub r15: usize,
	pub rflags: usize,
	pub rip: usize,
	pub cs: u16,
	pub ss: u16,
}

impl Registers
{
	pub const fn zero() -> Self
	{
		Self::new(0, 0, 0)
	}

	pub const fn new(rflags: usize, cs: u16, ss: u16) -> Self
	{
		Registers {
			rax: 0,
			rbx: 0,
			rcx: 0,
			rdx: 0,
			rbp: 0,
			rsp: 0,
			call_rsp: 0,
			call_save_rsp: 0,
			rdi: 0,
			rsi: 0,
			r8: 0,
			r9: 0,
			r10: 0,
			r11: 0,
			r12: 0,
			r13: 0,
			r14: 0,
			r15: 0,
			rflags,
			rip: 0,
			cs,
			ss,
		}
	}

	pub const fn from_rip(rip: usize) -> Self
	{
		let mut out = Self::zero();
		out.rip = rip;
		out
	}

	pub fn from_stack(stack: &Stack) -> Self
	{
		let mut out = Self::zero();
		out.apply_stack(stack);
		out
	}

	pub const fn from_priv(plevel: PrivLevel) -> Self
	{
		let mut out = Self::zero();
		out.apply_priv(plevel);
		out
	}

	pub fn apply_stack(&mut self, stack: &Stack) -> &mut Self
	{
		self.rsp = stack.top() - 8;
		self.rbp = 0;
		self
	}

	pub const fn apply_priv(&mut self, plevel: PrivLevel) -> &mut Self
	{
		match plevel {
			PrivLevel::Kernel => {
				self.rflags = 0x202;
				self.cs = 0x08;
				self.ss = 0x10;
			},
			PrivLevel::SuperUser => {
				self.rflags = 0x202;
				self.cs = 0x23;
				self.ss = 0x1b;
			},
			PrivLevel::IOPriv => {
				// FIXME: temporarily setting IOPL to 3 for testing
				self.rflags = 0x3202;
				self.cs = 0x23;
				self.ss = 0x1b;
			},
			PrivLevel::User(_) => {
				self.rflags = 0x202;
				self.cs = 0x23;
				self.ss = 0x1b;
			},
		}
		self
	}
}

pub fn thread_c<'a>() -> UniqueRef<'a, Thread>
{
	// This is (sort of) safe to do (only not in timer interrupt) because thread that called it is guarenteed
	// to have state restored back to same as before if it is interrupted
	unsafe { tlist.lock()[ThreadState::Running].g(0).unbound() }
}

// FIXME: this locks too many locks, potential smp race condition
pub fn proc_c() -> Arc<Process>
{
	// panic safety: if this thrad is running, the process exists
	thread_c().process().unwrap()
}

pub fn thread_get(tuid: Tuid) -> Option<ThreadRef> {
	let process = proc_get(tuid.pid())?;
	process.get_thread(tuid.tid())
}

pub fn proc_get(pid: Pid) -> Option<Arc<Process>>
{
	proc_list.lock().get(&pid).cloned()
}

pub fn block(state: ThreadState)
{
	match state {
		// do nothing if new state is stil running
		ThreadState::Running => return,
		_ => tlist.ensure(state),
	}

	thread_c().set_state(state);

	int_sched();
}

pub fn sleep_until(nsec: u64)
{
	block(ThreadState::Sleep(nsec));
}

pub fn sleep(duration: Duration)
{
	sleep_until(timer.nsec() + duration.as_nanos() as u64);
}

pub fn init() -> Result<(), Err>
{
	// allow execute disable in pages
	let efer_msr = rdmsr(EFER_MSR);
	wrmsr(EFER_MSR, efer_msr | EFER_EXEC_DISABLE);

	let kernel_proc = Process::new(PrivLevel::Kernel, "kernel".to_string(), "kernel".to_string());

	kernel_proc.new_thread(thread_cleaner as usize, Some("thread_cleaner".to_string()))?;

	// rip will be set on first context switch
	let idle_thread = Thread::from_stack(
		Arc::downgrade(&kernel_proc),
		kernel_proc.next_tid(),
		"idle_thread".to_string(),
		Registers::zero(),
		*INIT_STACK,
	)?;
	idle_thread.set_state(ThreadState::Running);
	tlist.lock()[ThreadState::Running].push(unsafe { idle_thread.clone() });

	kernel_proc.insert_thread(idle_thread);
	proc_list.lock().insert(kernel_proc.pid(), kernel_proc);

	Handler::Last(time_handler).register(IRQ_TIMER)?;
	Handler::Last(int_handler).register(INT_SCHED)?;

	Ok(())
}
