use spin::Mutex;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};
use crate::uses::*;
use crate::int::idt::{Handler, IRQ_TIMER, INT_SCHED};
use crate::util::{LinkedList, IMutex};
use crate::arch::x64::{cli_safe, sti_safe, sti_inc, rdmsr, wrmsr, EFER_MSR, EFER_EXEC_DISABLE};
use crate::time::timer;
use crate::upriv::PrivLevel;
use crate::consts::INIT_STACK;
use process::Process;
use thread::{Thread, ThreadLNode, ThreadState};
use core::ops::{Index, IndexMut};

// TODO: the scheduler uses a bad choice of data structures, i'll implement better ones later
// ready list should be changed once I decide what scheduling algorithm to use
// sleeping threads should be min heap or min tree
// futex list should be hash map or binary tree of linkedlists
// join should probably have a list in each thread that says all the threads that are waiting on them

// TODO: decide on locking api (function or regular mutex) and discard the other

mod process;
mod thread;

static tlist: IMutex<ThreadList> = IMutex::new (ThreadList::new ());
static proc_list: Mutex<BTreeMap<usize, Arc<Process>>> = Mutex::new (BTreeMap::new ());

// amount of time that elapses before we will switch to a new thread in nanoseconds
// current value is 50 milliseconds
const SCHED_TIME: u64 = 50000000;
static last_switch_nsec: AtomicU64 = AtomicU64::new (0);

// TODO: make this cpu local data
// FIXME: this is a bad way to return registers, and won't be safe with smp
static mut out_regs: Registers = Registers::new (0, 0, 0);

fn time_handler (regs: &Registers, _: u64) -> Option<&Registers>
{
	lock ();
	rprintln! ("hi");

	let mut out = None;

	let nsec = timer.nsec_no_latch ();
	if nsec - last_switch_nsec.load (Ordering::Relaxed) > SCHED_TIME
	{
		out = schedule (regs);
	}

	// returning from interrupt handler will set old flags
	defer_unlock ();

	out
}

fn int_handler (regs: &Registers, _: u64) -> Option<&Registers>
{
	lock ();

	let out = schedule (regs);

	// returning from interrupt handler will set old flags
	defer_unlock ();

	out
}

// schedule takes in current registers, and picks new thread to switch to
// if it doesn't, schedule returns None and does nothing with regs
// if it does, schedule sets the old thread's registers to be regs, switches address
// space if necessary, and returns the new thread's registers
// schedule will disable interrupts if necessary
fn schedule (regs: &Registers) -> Option<&Registers>
{
	let mut thread_list = tlist.lock ();

	let tpointer = loop
	{
		match thread_list[ThreadState::Ready].pop ()
		{
			Some(t) => {
				if !t.is_alive ()
				{
					t.move_to (ThreadState::Destroy, &mut thread_list);
				}
				else
				{
					break t;
				}
			},
			None => return None,
		}
	};

	let old_thread = thread_list[ThreadState::Running].pop ().expect ("no currently running thread");
	if let ThreadState::Running = old_thread.state ()
	{
		old_thread.set_state (ThreadState::Ready);
	}
	old_thread.insert_into (&mut thread_list);

	// TODO: add premptive multithreading here
	tpointer.set_state (ThreadState::Running);
	tpointer.insert_into (&mut thread_list);
	let new_process = tpointer.thread ().unwrap ().process ();

	if !old_thread.is_alive () || old_thread.thread ().unwrap ().process ().pid () != new_process.pid ()
	{
		unsafe { new_process.addr_space.load (); }
	}

	unsafe
	{
		// FIXME: ugly
		// safe to do if the scheduler is locked until returning from interrupt handler, since the thread can't be freed
		out_regs = *tpointer.thread ().unwrap ().regs.lock ();
		Some(&out_regs)
	}
}

// TODO: when smp addded, change these
fn lock ()
{
	cli_safe ();
}

fn unlock ()
{
	sti_safe ();
}

fn defer_unlock ()
{
	sti_inc ();
}

fn thread_cleaner ()
{
	loop
	{
		{
			let list = &mut tlist.lock ()[ThreadState::Destroy];
			while let Some(tpointer) = list.pop_front ()
			{
				unsafe
				{
					tpointer.dealloc ();
				}
			}
		}
		rprintln! ("end thread_cleaner loop");
	}
}

#[derive(Debug)]
pub struct ThreadList([LinkedList<ThreadLNode>; 4]);

impl ThreadList
{
	const fn new () -> Self
	{
		ThreadList([
			LinkedList::new (),
			LinkedList::new (),
			LinkedList::new (),
			LinkedList::new (),
		])
	}

	fn get (&self, state: ThreadState) -> Option<&LinkedList<ThreadLNode>>
	{
		match state
		{
			ThreadState::Running => Some(&self.0[0]),
			ThreadState::Ready => Some(&self.0[1]),
			ThreadState::Destroy => Some(&self.0[2]),
			ThreadState::Sleep(_) => Some(&self.0[3]),
			_ => None
		}
	}

	fn get_mut (&mut self, state: ThreadState) -> Option<&mut LinkedList<ThreadLNode>>
	{
		match state
		{
			ThreadState::Running => Some(&mut self.0[0]),
			ThreadState::Ready => Some(&mut self.0[1]),
			ThreadState::Destroy => Some(&mut self.0[2]),
			ThreadState::Sleep(_) => Some(&mut self.0[3]),
			_ => None
		}
	}
}

impl Index<ThreadState> for ThreadList
{
	type Output = LinkedList<ThreadLNode>;

	fn index (&self, state: ThreadState) -> &Self::Output
	{
		self.get (state).expect ("attempted to index ThreadState with invalid state")
	}
}

impl IndexMut<ThreadState> for ThreadList
{
	fn index_mut (&mut self, state: ThreadState) -> &mut Self::Output
	{
		self.get_mut (state).expect ("attempted to index ThreadState with invalid state")
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
	const fn new (rflags: usize, cs: u16, ss: u16) -> Self
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
}

pub fn thread_c () -> Arc<Thread>
{
	tlist.lock ()[ThreadState::Running][0].thread ().unwrap ()
}

pub fn proc_c () -> Arc<Process>
{
	thread_c ().process ()
}

pub fn init () -> Result<(), Err>
{
	// allow execute disable in pages
	let efer_msr = rdmsr (EFER_MSR);
	wrmsr (EFER_MSR, efer_msr | EFER_EXEC_DISABLE);

	let kernel_proc = Process::new (PrivLevel::Kernel, "kernel".to_string ());

	kernel_proc.new_thread (thread_cleaner)?;

	// rip will be set on first context switch
	let idle_thread = Thread::from_stack (Arc::downgrade (&kernel_proc), kernel_proc.next_tid (), "idle_thread".to_string (), 0, *INIT_STACK)?;
	let tpointer = ThreadLNode::new (Arc::downgrade (&idle_thread));
	tpointer.set_state (ThreadState::Running);
	tlist.lock ()[ThreadState::Running].push (tpointer);

	kernel_proc.insert_thread (idle_thread);
	proc_list.lock ().insert (kernel_proc.pid (), kernel_proc);

	Handler::Last(time_handler).register (IRQ_TIMER)?;
	Handler::Last(int_handler).register (INT_SCHED)?;

	Ok(())
}
