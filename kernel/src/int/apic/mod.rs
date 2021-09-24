use crate::uses::*;
use modular_bitfield::BitfieldSpecifier;
use crate::kdata::cpud;
use crate::acpi::madt::{Madt, MadtElem};

pub mod lapic;
pub mod ioapic;

pub use lapic::LocalApic;
pub use ioapic::IoApic;

#[derive(Debug, Clone, Copy, BitfieldSpecifier)]
#[bits = 3]
enum DelivMode {
	Fixed = 0,
	// only available for io apic and ipi
	// avoid for ipi
	LowPrio = 1,
	// avoid for ipi
	Smi = 2,
	Nmi = 4,
	Init = 5,
	// only available for ipi
	Sipi = 6,
	ExtInt = 7,
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
enum PinPolarity {
	ActiveHigh = 0,
	ActiveLow = 1,
}

#[derive(Debug, Clone, Copy, BitfieldSpecifier)]
enum RemoteIrr {
	None = 0,
	Servicing = 1,
}

pub unsafe fn init(madt: &Madt) {
	let mut lapic_addr = madt.lapic_addr as u64;

	for entry in madt.iter() {
		eprintln!("{:?}", entry);
		match entry {
			MadtElem::LocalApicOverride(data) => lapic_addr = data.addr,
			_ => (),
		}
	}

	*cpud().lapic.lock() = Some(LocalApic::from(PhysAddr::new(lapic_addr)));
}
