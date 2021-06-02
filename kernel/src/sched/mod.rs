use spin::Mutex;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use core::time::Duration;
use core::ops::{Index, IndexMut};
use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};
use crate::uses::*;
use crate::int::idt::{Handler, IRQ_TIMER, INT_SCHED};
use crate::util::{LinkedList, IMutex, UniqueRef, UniqueMut, UniquePtr};
use crate::arch::x64::{cli_safe, sti_safe, sti_inc, rdmsr, wrmsr, EFER_MSR, EFER_EXEC_DISABLE};
use crate::time::timer;
use crate::upriv::PrivLevel;
use crate::consts::INIT_STACK;
use crate::gdt::tss;
pub use process::Process;
pub use thread::{Thread, TNode, ThreadState};

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

pub mod sys;
mod process;
mod thread;
mod elf;

static tlist: IMutex<ThreadList> = IMutex::new (ThreadList::new ());
static proc_list: Mutex<BTreeMap<usize, Arc<Process>>> = Mutex::new (BTreeMap::new ());

// amount of time that elapses before we will switch to a new thread in nanoseconds
// current value is 100 milliseconds
const SCHED_TIME: u64 = 100000000;
static last_switch_nsec: AtomicU64 = AtomicU64::new (0);

// TODO: make this cpu local data
// FIXME: this is a bad way to return registers, and won't be safe with smp
static mut out_regs: Registers = Registers::new (0, 0, 0);

// These interrupt handlers, and all other timer interrupt handlers must not:
// lock any reguler mutexes or spinlocks (only IMutex)
// this means no memory allocation
fn time_handler (regs: &Registers, _: u64) -> Option<&Registers>
{
	lock ();
	rprintln! ("int");

	let mut out = None;

	let nsec = timer.nsec_no_latch ();

	let mut thread_list = tlist.lock ();
	let time_list = &mut thread_list[ThreadState::Sleep(0)];

	// FIXME: ugly
	for tpointer in unsafe { unbound_mut (time_list).iter () }
	{
		let sleep_nsec = tpointer.state ().sleep_nsec ().unwrap ();
		if nsec >= sleep_nsec
		{
			TNode::move_to (tpointer, ThreadState::Ready, Some(&mut thread_list), None).unwrap ();
		}
	}

	// release mutex
	drop (thread_list);

	if nsec - last_switch_nsec.load (Ordering::Relaxed) > SCHED_TIME
	{
		out = schedule (regs, nsec);
	}

	// returning from interrupt handler will set old flags
	defer_unlock ();

	out
}

// FIXME: this could potentially have a thread switch occur before lock
// when this happens, switching back to this thread will cause it to immediataly give up control again
// this is probably undesirable
fn int_handler (regs: &Registers, _: u64) -> Option<&Registers>
{
	lock ();

	let out = schedule (regs, timer.nsec ());

	// returning from interrupt handler will set old flags
	defer_unlock ();

	out
}

fn int_sched ()
{
	unsafe
	{
		asm!("int 128");
	}
}

// schedule takes in current registers, and picks new thread to switch to
// if it doesn't, schedule returns None and does nothing with regs
// if it does, schedule sets the old thread's registers to be regs, switches address
// space if necessary, and returns the new thread's registers
// schedule will disable interrupts if necessary
fn schedule (_regs: &Registers, nsec_current: u64) -> Option<&Registers>
{
	let mut thread_list = tlist.lock ();

	let tcell = loop
	{
		match thread_list[ThreadState::Ready].pop_front ()
		{
			Some(t) => {
				if !t.borrow ().is_alive ()
				{
					t.borrow_mut ().set_state (ThreadState::Destroy);
					TNode::insert_into (t, Some(&mut thread_list), None).unwrap ();
				}
				else
				{
					break t;
				}
			},
			None => return None,
		}
	};
	let mut tpointer = tcell.borrow_mut ();

	let old_thread_cell = thread_list[ThreadState::Running].pop ().expect ("no currently running thread");
	let mut old_thread = old_thread_cell.borrow_mut ();

	let nsec_last = last_switch_nsec.swap (nsec_current, Ordering::Relaxed);
	old_thread.inc_time (nsec_current - nsec_last);

	// FIXME: smp race condition
	let old_process = old_thread.thread ().unwrap ().process ();
	let mut tlproc_list = old_process.tlproc.lock ();

	if let ThreadState::Running = old_thread.state ()
	{
		old_thread.set_state (ThreadState::Ready);
	}
	drop (old_thread);
	// if process was dropped, move to destroy list
	let old_is_alive = match TNode::insert_into (old_thread_cell, Some(&mut thread_list), Some(&mut tlproc_list))
	{
		Ok(_) => true,
		Err(tcell) => {
			tcell.borrow_mut ().set_state (ThreadState::Destroy);
			TNode::insert_into (tcell, Some(&mut thread_list), None).unwrap ();
			false
		}
	};

	// TODO: add premptive multithreading here
	tpointer.set_state (ThreadState::Running);
	drop (tpointer);
	let tpointer = TNode::insert_into (tcell, Some(&mut thread_list), None).expect ("could not set running thread");

	rprintln! ("switching to:\n{:#x?}", *tpointer);

	let new_process = tpointer.thread ().unwrap ().process ();

	if !old_is_alive || old_process.pid () != new_process.pid ()
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
		// TODO: probably a good idea to put this logic in separate function
		// FIXME: there might be a race condition here (bochs freezes, but not qemu)
		loop
		{
			let mut thread_list = tlist.lock ();
			let tcell = match thread_list[ThreadState::Destroy].pop_front ()
			{
				Some(thread) => thread,
				None => break,
			};

			rprintln! ("Deallocing thread pointer:\n{:#x?}", tcell);

			// TODO: this is probably slow
			// avoid race condition with dealloc
			drop (thread_list);

			unsafe
			{
				tcell.borrow_mut ().dealloc ();
			}
		}
		rprintln! ("end thread_cleaner loop");
		thread_c ().sleep (Duration::new (1, 0));
	}
}

#[derive(Debug)]
pub struct ThreadList([LinkedList<TNode>; 4]);

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

	fn get (&self, state: ThreadState) -> Option<&LinkedList<TNode>>
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

	fn get_mut (&mut self, state: ThreadState) -> Option<&mut LinkedList<TNode>>
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
	type Output = LinkedList<TNode>;

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

pub fn thread_c<'a> () -> UniqueRef<'a, TNode>
{
	// This is (sort of) safe to do (only not in timer interrupt) because thread that called it is guarenteed
	// to have state restored back to same as before if it is interrupted
	unsafe
	{
		tlist.lock ()[ThreadState::Running].g (0).unbound ()
	}
}

// FIXME: potential smp race condition if process is dropped
pub fn thread_res_c () -> Arc<Thread>
{
	tlist.lock ()[ThreadState::Running].g (0).thread ().unwrap ()
}

// FIXME: this locks too many locks, potential smp race condition
pub fn proc_c () -> Arc<Process>
{
	thread_res_c ().process ()
}

pub fn init () -> Result<(), Err>
{
	assert! (core::mem::size_of::<ThreadState> () <= 16, "ThreadState is to big to fit in an AtomicU128");

	// allow execute disable in pages
	let efer_msr = rdmsr (EFER_MSR);
	wrmsr (EFER_MSR, efer_msr | EFER_EXEC_DISABLE);

	let kernel_proc = Process::new (PrivLevel::Kernel, "kernel".to_string ());

	kernel_proc.new_thread (thread_cleaner, Some("thread_cleaner".to_string ()))?;

	// rip will be set on first context switch
	let idle_thread = Thread::from_stack (Arc::downgrade (&kernel_proc), kernel_proc.next_tid (), "idle_thread".to_string (), 0, *INIT_STACK)?;
	let tpointer = TNode::new (Arc::downgrade (&idle_thread));
	tpointer.borrow_mut ().set_state (ThreadState::Running);
	tlist.lock ()[ThreadState::Running].push (tpointer);

	kernel_proc.insert_thread (idle_thread);
	proc_list.lock ().insert (kernel_proc.pid (), kernel_proc);

	Handler::Last(time_handler).register (IRQ_TIMER)?;
	Handler::Last(int_handler).register (INT_SCHED)?;

	Ok(())
}
