use crate::uses::*;
use crate::util::{HwaIter, HwaTag};
use super::{SdtHeader, Sdt};

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Madt {
	header: SdtHeader,
	lapic_addr: u32,
	lapic_flags: u32,
}

impl Madt {
	pub fn iter(&self) -> HwaIter<MadtTag> {
		unsafe {
			HwaIter::from_struct(self, self.header.size())
		}
	}
}

impl Sdt for Madt {
	fn header(&self) -> &SdtHeader {
		&self.header
	}
}

#[derive(Debug, Clone, Copy)]
pub enum MadtElem<'a> {
	ProcLocalApic(&'a ProcLocalApic),
	IoApic(&'a IoApic),
	IoApicSrcOverride(&'a IoApicSrcOverride),
	IoApicNmi(&'a IoApicNmi),
	LocalApicNmi(&'a LocalApicNmi),
	LocalApicOverride(&'a LocalApicOverride),
	ProcLocalX2Apic(&'a ProcLocalX2Apic),
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct MadtTag {
	typ: u8,
	size: u8,
}

impl HwaTag for MadtTag {
	type Elem<'a> = MadtElem<'a>;

	fn size(&self) -> usize {
		self.size as usize
	}

	fn elem(&self) -> Self::Elem<'_> {
		unsafe { match self.typ {
			0 => MadtElem::ProcLocalApic(self.raw_data()),
			1 => MadtElem::IoApic(self.raw_data()),
			2 => MadtElem::IoApicSrcOverride(self.raw_data()),
			3 => MadtElem::IoApicNmi(self.raw_data()),
			4 => MadtElem::LocalApicNmi(self.raw_data()),
			5 => MadtElem::LocalApicOverride(self.raw_data()),
			9 => MadtElem::ProcLocalX2Apic(self.raw_data()),
			_ => panic!("invalid madt type"),
		}}
	}
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ProcLocalApic {
	proc_id: u8,
	apic_id: u8,
	flags: u32,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct IoApic {
	ioapic_id: u8,
	reserved: u8,
	ioapic_addr: u32,
	global_sysint_base: u32,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct IoApicSrcOverride {
	bus_src: u8,
	irq_src: u8,
	global_sysint: u32,
	flags: u16,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct IoApicNmi {
	nmi_src: u8,
	reserved: u8,
	flags: u16,
	global_sysint: u32,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct LocalApicNmi {
	proc_id: u8,
	flags: u16,
	lint: u8,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct LocalApicOverride {
	reserved: u16,
	addr: u64,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ProcLocalX2Apic {
	reserved: u16,
	x2_apic_id: u32,
	flags: u32,
	acpi_id: u32,
}
