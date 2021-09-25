use crate::uses::*;
use core::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use modular_bitfield::BitfieldSpecifier;
use crate::kdata::cpud;
use crate::acpi::madt::{Madt, MadtElem};
use crate::util::IMutex;
use crate::int::idt::{irq_arr, IRQ_BASE, IRQ_TIMER};
use alloc::collections::BTreeMap;
use super::pic;

pub mod lapic;
pub mod ioapic;

use ioapic::{IrqEntry, IoApicDest};

pub use lapic::LocalApic;
pub use ioapic::IoApic;

// used to tell ap cores where their apic is
pub static LAPIC_ADDR: AtomicUsize = AtomicUsize::new(0);
pub static BSP_ID: AtomicU8 = AtomicU8::new(0);
pub static IO_APIC: IMutex<IoApic> = IMutex::new(unsafe { IoApic::new() });

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

// Default for when acpi tables say use default
impl Default for TriggerMode {
	fn default() -> Self {
		Self::Edge
	}
}

#[derive(Debug, Clone, Copy, BitfieldSpecifier)]
enum PinPolarity {
	ActiveHigh = 0,
	ActiveLow = 1,
}

// Default for when acpi tables say use default
impl Default for PinPolarity {
	fn default() -> Self {
		Self::ActiveHigh
	}
}

#[derive(Debug, Clone, Copy, BitfieldSpecifier)]
enum RemoteIrr {
	None = 0,
	Servicing = 1,
}

#[derive(Debug, Clone, Copy)]
struct IrqOverride {
	sysint: u32,
	polarity: PinPolarity,
	trigger_mode: TriggerMode,
}

#[derive(Debug)]
struct IrqOverrides {
	map: BTreeMap<u8, IrqOverride>,
}

impl IrqOverrides {
	const fn new() -> Self {
		IrqOverrides {
			map: BTreeMap::new(),
		}
	}

	fn get_irq(&self, irq: u8) -> IrqOverride {
		if let Some(irq) = self.map.get(&irq) {
			*irq
		} else {
			IrqOverride {
				sysint: irq as u32,
				polarity: PinPolarity::default(),
				trigger_mode: TriggerMode::default(),
			}
		}
	}

	fn override_irq(&mut self, irq: u8, over: IrqOverride) {
		self.map.insert(irq, over);
	}
}

// FIXME: correctly handle global sysint
pub unsafe fn init(madt: &Madt) {
	let mut lapic_addr = madt.lapic_addr as usize;

	// indicates the sytem has an 8259 pic that we have to disable
	// this is the only flags in flags, so I won't bother to make a bitflags for it
	if madt.lapic_flags & 1 > 0 {
		pic::disable();
	}
	
	// store irq overrides to make sure io apic is initialized first
	let mut overrides = IrqOverrides::new();

	// ap lapic ids
	let mut ap_ids = Vec::new();

	// if the bsp ProcLocalApic has been encountered yet
	let mut flag = true;

	for entry in madt.iter() {
		eprintln!("{:?}", entry);
		match entry {
			MadtElem::ProcLocalApic(data) => {
				if flag {
					BSP_ID.store(data.apic_id, Ordering::Release);
					flag = false;
				} else {
					ap_ids.push(data.apic_id);
				}
			}
			MadtElem::IoApic(io_apic) => {
				// to avoid warning about refernce to packed field
				let sysint = io_apic.global_sysint_base;
				// this will only be non zero in systems with multiple apics, which we do not support
				assert_eq!(sysint, 0);

				let ioapic_addr = PhysAddr::new(io_apic.ioapic_addr as u64);
				IO_APIC.lock().init(ioapic_addr);
			}
			MadtElem::IoApicSrcOverride(data) => {
				let polarity = match get_bits(data.flags as usize, 0..2) {
					0 => PinPolarity::default(),
					1 => PinPolarity::ActiveHigh,
					2 => panic!("invalid pin polarity flag in acpi tables"),
					3 => PinPolarity::ActiveLow,
					_ => unreachable!(),
				};

				let trigger_mode = match get_bits(data.flags as usize, 0..2) {
					0 => TriggerMode::default(),
					1 => TriggerMode::Edge,
					2 => panic!("invalid trigger mode flag in acpi tables"),
					3 => TriggerMode::Level,
					_ => unreachable!(),
				};

				let over = IrqOverride {
					sysint: data.global_sysint,
					polarity,
					trigger_mode,
				};

				overrides.override_irq(data.irq_src + IRQ_BASE, over);
			},
			MadtElem::LocalApicOverride(data) => lapic_addr = data.addr as usize,
			_ => (),
		}
	}

	let mut io_apic = IO_APIC.lock();

	for irq in irq_arr() {
		let over = overrides.get_irq(irq);
		let entry = if irq == IRQ_TIMER {
			IrqEntry::from(irq, IoApicDest::To(0), over.polarity, over.trigger_mode)
		} else {
			IrqEntry::from(irq, IoApicDest::To(BSP_ID.load(Ordering::Acquire)), over.polarity, over.trigger_mode)
		};
		io_apic.set_irq_entry(over.sysint as u8, entry);
	}

	drop(io_apic);

	LAPIC_ADDR.store(lapic_addr, Ordering::Release);
	*cpud().lapic.lock() = Some(LocalApic::from(PhysAddr::new(lapic_addr as u64)));
}
