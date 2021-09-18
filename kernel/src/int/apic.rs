use crate::uses::*;
use crate::acpi::madt::Madt;

pub unsafe fn init(madt: &Madt) {
	for entry in madt.iter() {
		eprintln!("{:?}", entry);
	}
}
