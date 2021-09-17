use crate::uses::*;
use core::slice;
use core::mem::transmute;
use crate::util::misc::phys_to_virt_usize;

pub mod madt;
pub mod hpet;

use madt::Madt;
use hpet::Hpet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdtType {
	// root system descriptor table
	Rsdt,
	// extended system descriptor table (64 bit version of rsdp)
	Xsdt,
	// multiple APIC description table
	Madt,
	// High precision event timer table
	Hpet,
}

#[derive(Debug, Clone, Copy)]
pub enum AcpiTable<'a> {
	Rsdt(&'a Rsdt),
	Madt(&'a Madt),
	Hpet(&'a Hpet),
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct SdtHeader {
	signature: [u8; 4],
	size: u32,
	revision: u8,
	checksum: u8,
	oemid: [u8; 6],
	oem_table_id: [u8; 8],
	oem_revision: u32,
	creator_id: u32,
	creator_revision: u32,
}

impl SdtHeader {
	pub fn size(&self) -> usize {
		self.size as usize
	}

	pub fn data_size(&self) -> usize {
		self.size() - size_of::<Self>()
	}

	pub fn data_ptr<T>(&self) -> *const T {
		unsafe {
			(self as *const Self).add(1) as *const T
		}
	}

	pub fn data<T>(&self) -> &[T] {
		if self.data_size() % size_of::<T>() != 0 {
			panic!("tried to get data slice of ACPI table and the size of elements in the slice did not evenly divide the size of the data");
		}
		unsafe {
			slice::from_raw_parts(self.data_ptr(), self.data_size() / size_of::<T>())
		}
	}

	// safety: length must be valid
	pub unsafe fn validate(&self) -> bool {
		let mut sum: usize = 0;
		let slice = slice::from_raw_parts(self as *const _ as *const u8, self.size());

		for n in slice {
			sum += *n as usize;
		}

		sum % 0x100 == 0
	}

	pub fn sdt_type(&self) -> Option<SdtType> {
		let s = &self.signature;
		// can't us match here
		Some(if s == "APIC".as_bytes() {
			SdtType::Madt
		} else if s == "RSDT".as_bytes() {
			SdtType::Rsdt
		} else if s == "XSDT".as_bytes() {
			SdtType::Xsdt
		} else if s == "HPET".as_bytes() {
			SdtType::Hpet
		} else {
			// TODO: add new acpi table types here
			return None;
		})
	}

	pub unsafe fn as_acpi_table(&self) -> Option<AcpiTable> {
		Some(match self.sdt_type()? {
			SdtType::Rsdt => {
				assert!(size_of::<Rsdt>() <= self.size());
				AcpiTable::Rsdt(transmute(self))
			},
			SdtType::Madt => {
				assert!(size_of::<Madt>() <= self.size());
				AcpiTable::Madt(transmute(self))
			},
			SdtType::Hpet => {
				assert!(size_of::<Hpet>() <= self.size());
				AcpiTable::Hpet(transmute(self))
			},
			_ => return None,
		})
	}
}

pub trait Sdt {
	fn header(&self) -> &SdtHeader;

	unsafe fn validate(&self) -> bool {
		self.header().validate()
	}

	fn sdt_type(&self) -> Option<SdtType> {
		self.header().sdt_type()
	}
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct Rsdt(SdtHeader);

impl Rsdt {
	// from a physical address
	pub unsafe fn from<'a>(addr: usize) -> Option<&'a Rsdt> {
		let out = (phys_to_virt_usize(addr) as *const Self).as_ref().unwrap();
		if !out.0.validate() {
			None
		} else {
			Some(out)
		}
	}

	// have to use a vec, not a slice, because the pointers are only 32 bits
	// safety: fields in rsdt must be valid
	pub unsafe fn tables(&self) -> Vec<AcpiTable> {
		let len = self.0.data_size() / 4;

		let mut out = Vec::with_capacity(len);

		let slice: &[u32] = self.0.data();
		for n in slice {
			let addr = phys_to_virt_usize(*n as usize);
			let table = (addr as *const SdtHeader).as_ref().unwrap();
			if let Some(table) = table.as_acpi_table() {
				out.push(table);
			}
		}
		out
	}

	// does not require memory allocation
	pub unsafe fn get_table(&self, table_type: SdtType) -> Option<AcpiTable> {
		let slice: &[u32] = self.0.data();
		for n in slice {
			let addr = phys_to_virt_usize(*n as usize);
			let table = (addr as *const SdtHeader).as_ref().unwrap();
			if let Some(typ) = table.sdt_type() {
				if typ == table_type {
					return table.as_acpi_table();
				}
			}
		}

		None
	}
}

impl Sdt for Rsdt {
	fn header(&self) -> &SdtHeader {
		&self.0
	}
}
