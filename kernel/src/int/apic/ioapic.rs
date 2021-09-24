use crate::uses::*;
use modular_bitfield::{bitfield, BitfieldSpecifier};
use super::*;

#[bitfield]
#[repr(u64)]
#[derive(Debug, Clone, Copy)]
struct IrqEntry {
	vec: u8,

	#[bits = 3]
	deliv_mode: DelivMode,

	#[bits = 1]
	dest_mode: DestMode,

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

	#[skip] __: B39,

	dest: u8,
}

pub struct IoApic {
	addr: usize,
}

impl IoApic {
	pub fn from(addr: PhysAddr) -> Self {
		let out = IoApic {
			addr: phys_to_virt(addr).as_u64() as usize,
		};
		out
	}
}
