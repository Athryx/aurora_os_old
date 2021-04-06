use spin::Mutex;
use crate::uses::*;
use crate::arch::x64::*;
use crate::sched::Regs;
use crate::int::idt::{Handler, IRQ_TIMER};

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
	pub static ref pit: Mutex<Pit> = Mutex::new (Pit::new (0xffff));
}

pub struct Pit
{
	// elapsed time since boot in nanoseconds
	elapsed_time: u64,
	// value pit counter resets too
	reset: u16,
	// nanosaconds elapsed per reset
	nano_reset: u64,
}

impl Pit
{
	fn new (reset: u16) -> Self
	{
		let mut out = Pit {
			elapsed_time: 0,
			reset: 0,
			nano_reset: 0,
		};
		out.set_reset (reset);
		out
	}

	pub fn set_reset (&mut self, ticks: u16)
	{
		// channel 0, low - high byte, square wave mode, 16 bit binary
		outb (PIT_COMMAND, 0b00110110);
		outb (PIT_CHANNEL_0, get_bits (ticks as _, 0..8) as _);
		outb (PIT_CHANNEL_0, get_bits (ticks as _, 8..16) as _);

		self.reset = ticks;
		self.nano_reset = NANOSEC_PER_CLOCK * ticks as u64;
	}

	pub fn tick (&mut self)
	{
		self.elapsed_time += self.nano_reset;
	}

	pub fn nsec (&self) -> u64
	{
		// lock latch
		outb (PIT_COMMAND, 0);
		let low = inb (PIT_CHANNEL_0);
		let high = inb (PIT_CHANNEL_0);

		self.elapsed_time + (NANOSEC_PER_CLOCK * ((high as u64) << 8 | low as u64))
	}

	pub fn sec (&self) -> u64
	{
		(self.nsec () + 500000000) / 1000000000
	}

	// less accurate, but faster
	// it will be much more accurate if also running in a timer interrupt handler
	pub fn nsec_no_latch (&self) -> u64
	{
		self.elapsed_time
	}

	pub fn sec_no_latch (&self) -> u64
	{
		(self.nsec_no_latch () + 500000000) / 1000000000
	}
}

fn timer_irq_handler (_: &Regs, _: u64) -> Option<&Regs>
{
	pit.lock ().tick ();
	None
}

pub fn init () -> Result<(), Err>
{
	Handler::First(timer_irq_handler).register (IRQ_TIMER)?;
	Ok(())
}
