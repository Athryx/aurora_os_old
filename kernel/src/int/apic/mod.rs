use crate::uses::*;
use modular_bitfield::{bitfield, BitfieldSpecifier};
use core::ptr;
use crate::util::IMutex;
use crate::acpi::madt::Madt;

pub mod lapic;
pub mod ioapic;

pub use lapic::LocalApic;
pub use ioapic::IoApic;

// TODO: maybe combine with DeliveMode
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
enum DestMode {
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
	dest_mode: DestMode,

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
				out.set_dest_mode(DestMode::Physical);
				out.set_dest(dest);
			},
			IpiDest::OtherLogical(dest) => {
				out.set_dest_short(IpiDestShort::None);
				out.set_dest_mode(DestMode::Logical);
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
enum DelivMode {
	Fixed = 0,
	// only available for io apic
	LowPrio = 1,
	Smi = 2,
	Nmi = 4,
	Init = 5,
	ExtInt = 7,
}

#[derive(Debug, Clone, Copy, BitfieldSpecifier)]
enum PinPolarity {
	ActiveHigh = 0,
	ActiveLow = 1,
}

#[derive(Debug, Clone, Copy, BitfieldSpecifier)]
enum RemoteIrr {
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

pub unsafe fn init(madt: &Madt) {
	for entry in madt.iter() {
		eprintln!("{:?}", entry);
	}
}
