use crate::uses::*;
use modular_bitfield::{bitfield, BitfieldSpecifier};
use core::ptr;
use crate::util::IMutex;
use crate::acpi::madt::Madt;
use crate::int::idt::IRQ_TIMER;
use super::*;

#[bitfield]
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
struct LvtEntry {
	vec: u8,

	#[bits = 3]
	deliv_mode: DelivMode,

	#[skip] __: B1,

	// read only
	#[bits = 1]
	#[skip(setters)]
	deliv_status: DelivStatus,

	#[bits = 1]
	polarity: PinPolarity,

	// read only
	#[bits = 1]
	#[skip(setters)]
	remote_irr: RemoteIrr,

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

#[derive(Debug, Clone, Copy)]
enum LvtType {
	Timer(LvtEntry),
	MachineCheck(LvtEntry),
	Lint0(LvtEntry),
	Lint1(LvtEntry),
	Error(LvtEntry),
	Perf(LvtEntry),
	Thermal(LvtEntry),
}

impl LvtType {
	fn inner(&self) -> LvtEntry {
		match self {
			Self::Timer(entry) => *entry,
			Self::MachineCheck(entry) => *entry,
			Self::Lint0(entry) => *entry,
			Self::Lint1(entry) => *entry,
			Self::Error(entry) => *entry,
			Self::Perf(entry) => *entry,
			Self::Thermal(entry) => *entry,
		}
	}
}

#[derive(Debug)]
pub struct LocalApic {
	addr: usize,
}

impl LocalApic {
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
		let mut out = LocalApic {
			addr: phys_to_virt(addr).as_u64() as usize,
		};
		out.set_lvt(LvtType::Timer(LvtEntry::new_masked()));
		out.set_lvt(LvtType::MachineCheck(LvtEntry::new_masked()));
		out.set_lvt(LvtType::Lint0(LvtEntry::new_masked()));
		out.set_lvt(LvtType::Lint1(LvtEntry::new_masked()));
		// TODO: handle errors
		out.set_lvt(LvtType::Error(LvtEntry::new_masked()));
		out.set_lvt(LvtType::Perf(LvtEntry::new_masked()));
		out.set_lvt(LvtType::Thermal(LvtEntry::new_masked()));
		out
	}

	pub fn send_ipi(&mut self, ipi: Ipi) {
		let cmd_reg: CmdReg = ipi.into();
		self.write_reg_64(Self::CMD_BASE, cmd_reg.into())
	}

	pub fn eoi(&mut self) {
		self.write_reg_32(Self::EOI, 0)
	}

	fn set_lvt(&mut self, lvte: LvtType) {
		match lvte {
			LvtType::Timer(entry) => self.write_reg_32(Self::LVT_TIMER, entry.into()),
			LvtType::MachineCheck(entry) => self.write_reg_32(Self::LVT_MACHINE_CHECK, entry.into()),
			LvtType::Lint0(entry) => self.write_reg_32(Self::LVT_LINT0, entry.into()),
			LvtType::Lint1(entry) => self.write_reg_32(Self::LVT_LINT1, entry.into()),
			LvtType::Error(entry) => self.write_reg_32(Self::LVT_ERROR, entry.into()),
			LvtType::Perf(entry) => self.write_reg_32(Self::LVT_PERF, entry.into()),
			LvtType::Thermal(entry) => self.write_reg_32(Self::LVT_THERMAL, entry.into()),
		}
	}

	fn error(&self) -> u32 {
		self.read_reg_32(Self::ERROR)
	}

	fn read_reg_32(&self, reg: usize) -> u32 {
		let ptr = (self.addr + reg) as *const u32;
		unsafe {
			ptr::read_volatile(ptr)
		}
	}

	fn write_reg_32(&mut self, reg: usize, val: u32) {
		let ptr = (self.addr + reg) as *mut u32;
		unsafe {
			ptr::write_volatile(ptr, val);
		}
	}

	fn read_reg_64(&self, reg: usize) -> u64 {
		let high = self.read_reg_32(reg + Self::REG_OFFSET) as u64;
		let low = self.read_reg_32(reg) as u64;

		(high << 32) | low
	}

	// writes bytes in right order for send_ipi
	fn write_reg_64(&mut self, reg: usize, val: u64) {
		let low = get_bits(val as usize, 0..32) as u32;
		let high = get_bits(val as usize, 32..64) as u32;

		self.write_reg_32(reg + Self::REG_OFFSET, high);
		self.write_reg_32(reg, low);
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

	/*fn apic_id(&self) -> u32 {
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
	}*/
}
