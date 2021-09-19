use crate::uses::*;
use modular_bitfield::{bitfield, BitfieldSpecifier};
use core::ptr;
use crate::acpi::madt::Madt;
use crate::int::idt::IRQ_TIMER;

#[derive(Debug, Clone, Copy, BitfieldSpecifier)]
#[bits = 3]
enum IpiDelivMode {
	Fixed = 0,
	// avoid
	LowestPriority = 1,
	// avoid
	Smi = 2,
	Nmi = 4,
	Init = 5,
	Sipi = 6,
}

#[derive(Debug, Clone, Copy, BitfieldSpecifier)]
enum IpiDestMode {
	Physical = 0,
	Logical = 1,
}

#[derive(Debug, Clone, Copy, BitfieldSpecifier)]
enum DelivStatus {
	Idle = 0,
	Pending = 1,
}

#[derive(Debug, Clone, Copy, BitfieldSpecifier)]
enum TriggerMode {
	Edge = 0,
	// avoid for ipi
	Level = 1,
}

#[derive(Debug, Clone, Copy, BitfieldSpecifier)]
enum IpiDestShort {
	None = 0,
	This = 1,
	AllExcludeThis = 2,
	AllIncludeThis = 3,
}

#[bitfield]
#[repr(u64)]
pub struct CmdReg {
	vector: u8,

	#[bits = 3]
	deliv_mode: IpiDelivMode,

	#[bits = 1]
	dest_mode: IpiDestMode,

	// read only
	#[bits = 1]
	#[skip(setters)]
	status: DelivStatus,

	#[skip] __: B1,

	// true: assert
	// false: de assert
	// should always be true
	assert: bool,

	// should always be IpiTriggerMode::Edge
	#[bits = 1]
	trigger_mode: TriggerMode,

	#[skip] __: B2,

	#[bits = 2]
	dest_short: IpiDestShort,

	#[skip] __: B36,

	dest: u8,
}

impl Default for CmdReg {
	fn default() -> Self {
		Self::new()
			.with_assert(true)
			.with_trigger_mode(TriggerMode::Edge)
	}
}

impl From<Ipi> for CmdReg {
	fn from(ipi: Ipi) -> Self {
		let mut out = Self::default();

		match ipi.dest() {
			IpiDest::This => out.set_dest_short(IpiDestShort::This),
			IpiDest::AllExcludeThis => out.set_dest_short(IpiDestShort::AllExcludeThis),
			IpiDest::AllIncludeThis => out.set_dest_short(IpiDestShort::AllIncludeThis),
			IpiDest::OtherPhysical(dest) => {
				out.set_dest_short(IpiDestShort::None);
				out.set_dest_mode(IpiDestMode::Physical);
				out.set_dest(dest);
			},
			IpiDest::OtherLogical(dest) => {
				out.set_dest_short(IpiDestShort::None);
				out.set_dest_mode(IpiDestMode::Logical);
				out.set_dest(dest);
			},
		}

		match ipi {
			Ipi::To(_, vec) => {
				out.set_vector(vec);
				out.set_deliv_mode(IpiDelivMode::Fixed);
			},
			Ipi::Smi(_) => {
				out.set_vector(0);
				out.set_deliv_mode(IpiDelivMode::Smi);
			},
			Ipi::Init(_) => {
				out.set_vector(0);
				out.set_deliv_mode(IpiDelivMode::Init);
			},
			Ipi::Sipi(_, vec) => {
				out.set_vector(vec);
				out.set_deliv_mode(IpiDelivMode::Sipi);
			},
		}

		out
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpiDest {
	This,
	AllExcludeThis,
	AllIncludeThis,
	OtherPhysical(u8),
	OtherLogical(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ipi {
	To(IpiDest, u8),
	Smi(IpiDest),
	Init(IpiDest),
	Sipi(IpiDest, u8),
}

impl Ipi {
	pub fn dest(&self) -> IpiDest {
		match *self {
			Self::To(dest, _) => dest,
			Self::Smi(dest) => dest,
			Self::Init(dest) => dest,
			Self::Sipi(dest, _) => dest,
		}
	}
}

#[derive(Debug, Clone, Copy, BitfieldSpecifier)]
#[bits = 3]
enum LvtDelivMode {
	Fixed = 0,
	Smi = 2,
	Nmi = 4,
	Init = 5,
	ExtInt = 7,
}

#[derive(Debug, Clone, Copy, BitfieldSpecifier)]
enum LvtPinPolarity {
	ActiveHigh = 0,
	ActiveLow = 1,
}

#[derive(Debug, Clone, Copy, BitfieldSpecifier)]
enum LvtRemoteIrr {
	None = 0,
	Servicing = 1,
}

#[derive(Debug, Clone, Copy, BitfieldSpecifier)]
#[bits = 2]
enum LvtTimerMode {
	OneShot = 0,
	Periodic = 1,
	TscDeadline = 2,
}

#[bitfield]
#[repr(u32)]
struct LvtEntry {
	vec: u8,

	#[bits = 3]
	deliv_mode: LvtDelivMode,

	#[skip] __: B1,

	// read only
	#[bits = 1]
	#[skip(setters)]
	deliv_status: DelivStatus,

	#[bits = 1]
	polarity: LvtPinPolarity,

	// read only
	#[bits = 1]
	#[skip(setters)]
	remote_irr: LvtRemoteIrr,

	#[bits = 1]
	trigger_mode: TriggerMode,

	masked: bool,

	#[bits = 2]
	timer_mode: LvtTimerMode,

	#[skip] __: B13
}

impl LvtEntry {
	fn new_timer(vec: u8) -> Self {
		Self::default()
			.with_timer_mode(LvtTimerMode::Periodic)
			.with_vec(vec)
	}

	fn new_masked() -> Self {
		Self::default().with_masked(true)
	}
}

impl Default for LvtEntry {
	// use default instead of new just in case a flag needs to be set in the future
	fn default() -> Self {
		Self::new()
	}
}

#[derive(Debug)]
pub struct ApicRegs(usize);

impl ApicRegs {
	// offset between registers
	const REG_OFFSET: usize = 0x10;

	const APIC_ID: usize = 0x20;
	const APIC_VERSION: usize = 0x30;

	const TASK_PRIORITY: usize = 0x80;
	const ARBITRATION_PRIORITY: usize = 0x90;
	const PROC_PRIORITY: usize = 0xa0;

	const EOI: usize = 0xb0;

	const REMOTE_READ: usize = 0xc0;

	const LOGICAL_DEST: usize = 0xd0;
	const DEST_FORMAT: usize = 0xe0;

	const SPURIOUS_VEC: usize = 0xf0;

	// 256 bit register
	const IN_SERVICE_BASE: usize = 0x100;

	// 256 bit register
	const TRIGGER_MODE_BASE: usize = 0x180;

	// 256 bit register
	const IRQ_BASE: usize = 0x200;

	const ERROR: usize = 0x280;

	const LVT_MACHINE_CHECK: usize = 0x2f0;
	
	// 64 bit register
	const CMD_BASE: usize = 0x300;

	const LVT_TIMER: usize = 0x320;
	const LVT_THERMAL: usize = 0x330;
	const LVT_PERF: usize = 0x340;
	const LVT_LINT0: usize = 0x350;
	const LVT_LINT1: usize = 0x360;
	const LVT_ERROR: usize = 0x370;

	const TIMER_INIT_COUNT: usize = 0x380;
	const TIMER_COUNT: usize = 0x390;
	const TIMER_DIVIDE_CONFIG: usize = 0x3e0;

	pub fn from(addr: PhysAddr) -> Self {
		let mut out = ApicRegs(phys_to_virt(addr).as_u64() as usize);
		out.set_lvt_timer(LvtEntry::new_timer(IRQ_TIMER));
		out.set_lvt_machine_check(LvtEntry::new_masked());
		out.set_lvt_lint0(LvtEntry::new_masked());
		out.set_lvt_lint1(LvtEntry::new_masked());
		// TODO: handle errors
		out.set_lvt_error(LvtEntry::new_masked());
		out.set_lvt_perf(LvtEntry::new_masked());
		out.set_lvt_thermal(LvtEntry::new_masked());
		out
	}

	pub fn send_ipi(&mut self, ipi: Ipi) {
		let cmd_reg: CmdReg = ipi.into();
		self.set_cmd(cmd_reg.into())
	}

	pub fn eoi(&mut self) {
		self.write_reg_32(Self::EOI, 0)
	}

	fn read_reg_32(&self, reg: usize) -> u32 {
		let ptr = (self.0 + reg) as *const u32;
		unsafe {
			ptr::read_volatile(ptr)
		}
	}

	fn write_reg_32(&mut self, reg: usize, val: u32) {
		let ptr = (self.0 + reg) as *mut u32;
		unsafe {
			ptr::write_volatile(ptr, val);
		}
	}

	fn read_reg_64(&self, reg: usize) -> u64 {
		let ptr_low = (self.0 + reg) as *const u32;
		let ptr_high = (self.0 + reg + Self::REG_OFFSET) as *const u32;

		let low = unsafe {
			ptr::read_volatile(ptr_low) as u64
		};
		let high = unsafe {
			ptr::read_volatile(ptr_high) as u64
		};

		(high << 32) | low
	}

	// writes bytes in right order for send_ipi
	fn write_reg_64(&self, reg: usize, val: u64) {
		let ptr_low = (self.0 + reg) as *mut u32;
		let ptr_high = (self.0 + reg + Self::REG_OFFSET) as *mut u32;

		let low = get_bits(val as usize, 0..32) as u32;
		let high = get_bits(val as usize, 32..64) as u32;

		unsafe {
			ptr::write_volatile(ptr_high, high);
			ptr::write_volatile(ptr_low, low);
		}
	}

	fn read_reg_256(&self, reg: usize) -> [u64; 4] {
		let mut out = [0; 4];
		for (i, elem) in out.iter_mut().enumerate() {
			*elem = self.read_reg_64(reg + 2 * i * Self::REG_OFFSET);
		}
		out
	}

	fn write_reg_256(&mut self, reg: usize, val: [u64; 4]) {
		for (i, elem) in val.iter().enumerate() {
			self.write_reg_64(reg + 2 * i * Self::REG_OFFSET, *elem);
		}
	}

	fn apic_id(&self) -> u32 {
		self.read_reg_32(Self::APIC_ID)
	}

	fn apic_version(&self) -> u32 {
		self.read_reg_32(Self::APIC_VERSION)
	}

	fn task_priority(&self) -> u32 {
		self.read_reg_32(Self::TASK_PRIORITY)
	}

	fn set_task_priority(&mut self, val: u32) {
		self.write_reg_32(Self::TASK_PRIORITY, val)
	}

	fn arbitration_priority(&self) -> u32 {
		self.read_reg_32(Self::ARBITRATION_PRIORITY)
	}

	fn proc_priority(&self) -> u32 {
		self.read_reg_32(Self::PROC_PRIORITY)
	}

	fn set_eoi(&mut self, val: u32) {
		self.write_reg_32(Self::EOI, val)
	}

	fn remote_read(&self) -> u32 {
		self.read_reg_32(Self::REMOTE_READ)
	}

	fn logical_dest(&self) -> u32 {
		self.read_reg_32(Self::LOGICAL_DEST)
	}

	fn set_logical_dest(&mut self, val: u32) {
		self.write_reg_32(Self::LOGICAL_DEST, val)
	}

	fn dest_format(&self) -> u32 {
		self.read_reg_32(Self::DEST_FORMAT)
	}

	fn set_dest_format(&mut self, val: u32) {
		self.write_reg_32(Self::DEST_FORMAT, val)
	}

	fn spurious_vec(&self) -> u32 {
		self.read_reg_32(Self::SPURIOUS_VEC)
	}

	fn set_spurious_vec(&mut self, val: u32) {
		self.write_reg_32(Self::SPURIOUS_VEC, val)
	}

	fn error(&self) -> u32 {
		self.read_reg_32(Self::ERROR)
	}

	fn cmd(&self) -> u64 {
		self.read_reg_64(Self::CMD_BASE)
	}

	fn set_cmd(&mut self, val: u64) {
		self.write_reg_64(Self::CMD_BASE, val)
	}

	fn lvt_machine_check(&self) -> u32 {
		self.read_reg_32(Self::LVT_MACHINE_CHECK)
	}

	fn set_lvt_machine_check(&mut self, val: LvtEntry) {
		self.write_reg_32(Self::LVT_MACHINE_CHECK, val.into())
	}

	fn lvt_timer(&self) -> u32 {
		self.read_reg_32(Self::LVT_TIMER)
	}

	fn set_lvt_timer(&mut self, val: LvtEntry) {
		self.write_reg_32(Self::LVT_TIMER, val.into())
	}

	fn lvt_thermal(&self) -> u32 {
		self.read_reg_32(Self::LVT_THERMAL)
	}

	fn set_lvt_thermal(&mut self, val: LvtEntry) {
		self.write_reg_32(Self::LVT_THERMAL, val.into())
	}

	fn lvt_perf(&self) -> u32 {
		self.read_reg_32(Self::LVT_PERF)
	}

	fn set_lvt_perf(&mut self, val: LvtEntry) {
		self.write_reg_32(Self::LVT_PERF, val.into())
	}

	fn lvt_lint0(&self) -> u32 {
		self.read_reg_32(Self::LVT_LINT0)
	}

	fn set_lvt_lint0(&mut self, val: LvtEntry) {
		self.write_reg_32(Self::LVT_LINT0, val.into())
	}

	fn lvt_lint1(&self) -> u32 {
		self.read_reg_32(Self::LVT_LINT1)
	}

	fn set_lvt_lint1(&mut self, val: LvtEntry) {
		self.write_reg_32(Self::LVT_LINT1, val.into())
	}

	fn lvt_error(&self) -> u32 {
		self.read_reg_32(Self::LVT_ERROR)
	}

	fn set_lvt_error(&mut self, val: LvtEntry) {
		self.write_reg_32(Self::LVT_ERROR, val.into())
	}

	fn timer_init_count(&self) -> u32 {
		self.read_reg_32(Self::TIMER_INIT_COUNT)
	}

	fn set_timer_init_count(&mut self, val: u32) {
		self.write_reg_32(Self::TIMER_INIT_COUNT, val)
	}

	fn timer_count(&self) -> u32 {
		self.read_reg_32(Self::TIMER_COUNT)
	}

	fn timer_divide_config(&self) -> u32 {
		self.read_reg_32(Self::TIMER_DIVIDE_CONFIG)
	}

	fn set_timer_divide_config(&mut self, val: u32) {
		self.write_reg_32(Self::TIMER_DIVIDE_CONFIG, val)
	}
}

pub unsafe fn init(madt: &Madt) {
	for entry in madt.iter() {
		eprintln!("{:?}", entry);
	}
}
