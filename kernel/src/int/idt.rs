use concat_idents::concat_idents;

use crate::uses::*;
use crate::sched::{thread_c, Registers};
use crate::{gdt, kdata};
use crate::arch::x64::CPUPrivLevel;
use super::pic::{PICM_OFFSET, PICS_OFFSET};

// TODO: maybe put these in an enum
pub const EXC_DIVIDE_BY_ZERO: u8 = 0;
pub const EXC_DEBUG: u8 = 1;
pub const EXC_NON_MASK_INTERRUPT: u8 = 2;
pub const EXC_BREAKPOINT: u8 = 3;
pub const EXC_OVERFLOW: u8 = 4;
pub const EXC_BOUND_RANGE_EXCEED: u8 = 5;
pub const EXC_INVALID_OPCODE: u8 = 6;
pub const EXC_DEVICE_UNAVAILABLE: u8 = 7;
pub const EXC_DOUBLE_FAULT: u8 = 8;
pub const EXC_NONE_9: u8 = 9;
pub const EXC_INVALID_TSS: u8 = 10;
pub const EXC_SEGMENT_NOT_PRESENT: u8 = 11;
pub const EXC_STACK_SEGMENT_FULL: u8 = 12;
pub const EXC_GENERAL_PROTECTION_FAULT: u8 = 13;
pub const EXC_PAGE_FAULT: u8 = 14;

pub const PAGE_FAULT_PROTECTION: u64 = 1;
pub const PAGE_FAULT_WRITE: u64 = 1 << 1;
pub const PAGE_FAULT_USER: u64 = 1 << 2;
pub const PAGE_FAULT_RESERVED: u64 = 1 << 3;
pub const PAGE_FAULT_EXECUTE: u64 = 1 << 4;

pub const EXC_NONE_15: u8 = 15;
pub const EXC_X87_FLOATING_POINT: u8 = 16;
pub const EXC_ALIGNMENT_CHECK: u8 = 17;
pub const EXC_MACHINE_CHECK: u8 = 18;
pub const EXC_SIMD_FLOATING_POINT: u8 = 19;
pub const EXC_VIRTUALIZATION: u8 = 20;
pub const EXC_NONE_21: u8 = 21;
pub const EXC_NONE_22: u8 = 22;
pub const EXC_NONE_23: u8 = 23;
pub const EXC_NONE_24: u8 = 24;
pub const EXC_NONE_25: u8 = 25;
pub const EXC_NONE_26: u8 = 26;
pub const EXC_NONE_27: u8 = 27;
pub const EXC_NONE_28: u8 = 28;
pub const EXC_NONE_29: u8 = 29;
pub const EXC_SECURITY: u8 = 30;
pub const EXC_NONE_31: u8 = 31;

pub const IRQ_BASE: u8 = PICM_OFFSET;

pub const IRQ_TIMER: u8 = PICM_OFFSET;
pub const IRQ_KEYBOARD: u8 = PICM_OFFSET + 1;
pub const IRQ_SERIAL_PORT_2: u8 = PICM_OFFSET + 3;
pub const IRQ_SERIAL_PORT_1: u8 = PICM_OFFSET + 4;
pub const IRQ_PARALLEL_PORT_2_3: u8 = PICM_OFFSET + 5;
pub const IRQ_FLOPPY_DISK: u8 = PICM_OFFSET + 6;
pub const IRQ_PARALLEL_PORT_1: u8 = PICM_OFFSET + 7;

pub const IRQ_CLOCK: u8 = PICS_OFFSET;
pub const IRQ_ACPI: u8 = PICS_OFFSET + 1;
pub const IRQ_NONE_1: u8 = PICS_OFFSET + 2;
pub const IRQ_NONE_2: u8 = PICS_OFFSET + 3;
pub const IRQ_MOUSE: u8 = PICS_OFFSET + 4;
pub const IRQ_CO_PROCESSOR: u8 = PICS_OFFSET + 5;
pub const IRQ_PRIMARY_ATA: u8 = PICS_OFFSET + 6;
pub const IRQ_SECONDARY_ATA: u8 = PICS_OFFSET + 7;

pub const INT_SCHED: u8 = 128;

// NOTE: on some processors, according to intel manuals, bits 0-3 of the spurious vector register are always 0,
// so we should always choose a spurious vector number with bits 0-3 zeroed
pub const SPURIOUS: u8 = 0xf0;

const MAX_HANDLERS: usize = 16;
const IDT_SIZE: usize = 256;

static mut idt: Idt = Idt::new();
static mut int_handlers: [[Option<IntHandlerFunc>; MAX_HANDLERS]; IDT_SIZE] =
	[[None; MAX_HANDLERS]; IDT_SIZE];

pub type IntHandlerFunc = fn(&Registers, u64) -> Option<&Registers>;

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
struct Idt([IdtEntry; IDT_SIZE]);

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct IdtPointer
{
	limit: u16,
	base: u64,
}

impl Idt
{
	const fn new() -> Self
	{
		Idt([IdtEntry::none(); IDT_SIZE])
	}

	fn load(&self)
	{
		let idtptr = IdtPointer {
			limit: (size_of::<Idt>() - 1) as _,
			base: (self as *const _) as _,
		};

		unsafe {
			asm!("lidt [{}]", in(reg) &idtptr, options(nostack));
		}
	}
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct IntData
{
	rip: u64,
	cs: u64,
	rflags: u64,
	rsp: u64,
	ss: u64,
}

pub enum IntHandlerType
{
	Interrupt,
	Trap,
}

impl IntHandlerType
{
	// get attr flags for IdtEntry
	fn get_attr_flags(&self, ring: CPUPrivLevel) -> u8
	{
		match self {
			Self::Interrupt => 0x80 | ring.n() << 5 | 0xe,
			Self::Trap => 0x80 | ring.n() << 5 | 0xf,
		}
	}
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct IdtEntry
{
	addr1: u16,
	// must be kernel code selector
	code_selector: u16,
	ist: u8,
	attr: u8,
	addr2: u16,
	addr3: u32,
	zero: u32,
}

impl IdtEntry
{
	fn new(addr: usize, htype: IntHandlerType, ring: CPUPrivLevel) -> Self
	{
		IdtEntry {
			addr1: get_bits(addr, 0..16) as _,
			addr2: get_bits(addr, 16..32) as _,
			addr3: get_bits(addr, 32..64) as _,
			code_selector: 8,
			ist: 0,
			attr: htype.get_attr_flags(ring),
			zero: 0,
		}
	}

	const fn none() -> Self
	{
		IdtEntry {
			addr1: 0,
			addr2: 0,
			addr3: 0,
			code_selector: 0,
			ist: 0,
			attr: 0,
			zero: 0,
		}
	}
}

pub enum Handler
{
	First(IntHandlerFunc),
	Normal(IntHandlerFunc),
	Last(IntHandlerFunc),
}

impl Handler
{
	// will never put normal in first or last position
	pub fn register(&self, vec: u8) -> Result<(), Err>
	{
		let vec = vec as usize;
		unsafe {
			match self {
				Self::First(func) => {
					if int_handlers[vec][0].is_none() {
						int_handlers[vec][0] = Some(*func);
						Ok(())
					} else {
						Err(Err::new("couldn't register int handler for first position"))
					}
				},
				Self::Normal(func) => {
					for i in 1..MAX_HANDLERS {
						if int_handlers[vec][i].is_none() {
							int_handlers[vec][i] = Some(*func);
							return Ok(());
						}
					}
					Err(Err::new(
						"couldn't register int handler for middle position",
					))
				},
				Self::Last(func) => {
					if int_handlers[vec][MAX_HANDLERS - 1].is_none() {
						int_handlers[vec][MAX_HANDLERS - 1] = Some(*func);
						Ok(())
					} else {
						Err(Err::new("couldn't register int handler for first position"))
					}
				},
			}
		}
	}
}

pub fn irq_arr() -> [u8; 15] {
	[IRQ_TIMER,
		IRQ_KEYBOARD,
		IRQ_SERIAL_PORT_2,
		IRQ_SERIAL_PORT_1,
		IRQ_PARALLEL_PORT_2_3,
		IRQ_FLOPPY_DISK,
		IRQ_PARALLEL_PORT_1,
		IRQ_CLOCK,
		IRQ_ACPI,
		IRQ_NONE_1,
		IRQ_NONE_2,
		IRQ_MOUSE,
		IRQ_CO_PROCESSOR,
		IRQ_PRIMARY_ATA,
		IRQ_SECONDARY_ATA]
}

#[no_mangle]
extern "C" fn rust_int_handler(vec: u8, regs: &mut Registers, error_code: u64)
	-> Option<&Registers>
{
	let vec = vec as usize;

	// set call_rsp and call_save_rsp in regs data structure which are not set by assembly
	{
		let data = kdata::cpud();
		regs.call_rsp = data.call_rsp;
		regs.call_save_rsp = data.call_save_rsp;
	}

	*thread_c().regs.lock() = *regs;

	let mut out = None;

	for i in 0..MAX_HANDLERS {
		let func = unsafe { int_handlers[vec][i] };
		if let Some(func) = func {
			if i == MAX_HANDLERS - 1 {
				out = func(regs, error_code);
			} else {
				func(regs, error_code);
			}
		}
	}

	if let Some(regs) = out {
		d();
		let mut tss = gdt::tss.lock();
		tss.rsp0 = regs.call_rsp as _;
		let mut data = kdata::cpud();
		data.call_rsp = regs.call_rsp;
		data.call_save_rsp = regs.call_save_rsp;
	} else {
		/*let thread = thread_c();
		let mut rcv_regs = thread.rcv_regs().lock();

		if let Ok(regs) = *rcv_regs {
			// don't need to set regs because these will be set at next interrupt
			use crate::sched::out_regs;
			unsafe {
				out_regs = regs;
				out = Some(&out_regs);
			}
			*rcv_regs = Err(SysErr::Unknown);
		}*/
	}

	out
}

macro_rules! minth {
	( $n:literal, $htype:expr, $ring:expr ) => {
		concat_idents! (fn_name = int_handler_, $n {
			extern "C" {
				fn fn_name ();
			}
			unsafe
			{
				idt.0[$n] = IdtEntry::new (fn_name as usize, $htype, $ring);
			}
		});
	}
}

pub fn init()
{
	// TODO: set IntHandlerType correctly
	minth!(0, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(1, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(2, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(3, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(4, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(5, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(6, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(7, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(8, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(9, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(10, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(11, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(12, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(13, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(14, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(15, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(16, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(17, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(18, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(19, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(20, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(21, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(22, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(23, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(24, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(25, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(26, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(27, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(28, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(29, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(30, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(31, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(32, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(33, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(34, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(35, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(36, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(37, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(38, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(39, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(40, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(41, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(42, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(43, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(44, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(45, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(46, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(47, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);
	minth!(128, IntHandlerType::Interrupt, CPUPrivLevel::Ring0);

	unsafe {
		idt.load();
	}
}
