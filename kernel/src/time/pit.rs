use spin::Mutex;
use core::sync::atomic::{AtomicU16, AtomicU64, Ordering};
use core::time::Duration;
use crate::uses::*;
use crate::arch::x64::*;
use crate::sched::Registers;
use crate::int::idt::{Handler, IRQ_TIMER};
use super::NANOSEC_PER_SEC;

const PIT_INTERRUPT_TERMINAL_COUNT: u8 = 0;
const PIT_ONE_SHOT: u8 = 1;
const PIT_RATE_GENERATOR: u8 = 2;
const PIT_SQUARE_WAVE: u8 = 3;
const PIT_SOFTWARE_STROBE: u8 = 4;
const PIT_HARDWARE_STROBE: u8 = 5;

const PIT_CHANNEL_0: u16 = 0x40;
const PIT_CHANNEL_1: u16 = 0x41;
const PIT_CHANNEL_2: u16 = 0x42;
const PIT_COMMAND: u16 = 0x43;

const NANOSEC_PER_CLOCK: u64 = 838;

lazy_static!
{
	pub static ref pit: Pit = Pit::new (0xffff);
}

pub struct Pit
{
	// elapsed time since boot in nanoseconds
	elapsed_time: AtomicU64,
	// value pit counter resets too
	reset: AtomicU16,
	// nanosaconds elapsed per reset
	nano_reset: AtomicU64,
	// needed for certain operations
	lock: Mutex<()>,
}

impl Pit
{
	fn new (reset: u16) -> Self
	{
		let out = Pit {
			elapsed_time: AtomicU64::new (0),
			reset: AtomicU16::new (0),
			nano_reset: AtomicU64::new (0),
			lock: Mutex::new (()),
		};
		out.set_reset (reset);
		out
	}

	// not safe to call from scheduler interrupt handler
	pub fn set_reset (&self, ticks: u16)
	{
		// channel 0, low - high byte, rate generator mode, 16 bit binary
		self.lock.lock ();
		outb (PIT_COMMAND, 0b00110100);
		outb (PIT_CHANNEL_0, get_bits (ticks as _, 0..8) as _);
		outb (PIT_CHANNEL_0, get_bits (ticks as _, 8..16) as _);

		self.reset.store (ticks, Ordering::Relaxed);
		self.nano_reset.store (NANOSEC_PER_CLOCK * ticks as u64, Ordering::Relaxed);
	}

	fn tick (&self)
	{
		self.elapsed_time.fetch_add(self.nano_reset.load (Ordering::Relaxed), Ordering::Relaxed);
	}

	pub fn nsec (&self) -> u64
	{
		if let Some(_lock) = self.lock.try_lock ()
		{
			// lock latch
			outb (PIT_COMMAND, 0);
			let low = inb (PIT_CHANNEL_0);
			let high = inb (PIT_CHANNEL_0);
			self.elapsed_time.load (Ordering::Relaxed) + (NANOSEC_PER_CLOCK * (self.reset.load (Ordering::Relaxed) as u64 - ((high as u64) << 8 | low as u64)))
		}
		else
		{
			// lower accuracy, but ensures no deadlocks in sheduler
			self.nsec_no_latch ()
		}
	}

	pub fn duration (&self) -> Duration
	{
		Duration::from_nanos (self.nsec ())
	}

	// less accurate, but faster
	// it will be much more accurate if also running in a timer interrupt handler
	pub fn nsec_no_latch (&self) -> u64
	{
		self.elapsed_time.load (Ordering::Relaxed)
	}

	pub fn duration_no_latch (&self) -> Duration
	{
		Duration::from_nanos (self.nsec_no_latch ())
	}
}

fn timer_irq_handler (_: &Registers, _: u64) -> Option<&Registers>
{
	pit.tick ();
	None
}

pub fn init () -> Result<(), Err>
{
	Handler::First(timer_irq_handler).register (IRQ_TIMER)?;
	Ok(())
}
