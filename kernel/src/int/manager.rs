use crate::uses::*;
use alloc::collections::BTreeMap;
use super::Irq;

// TODO: probably delete because this isn't needed
#[derive(Debug)]
pub struct IrqManager {
	map: BTreeMap<Irq, Irq>,
}

impl IrqManager {
	pub const fn new() -> Self {
		IrqManager {
			map: BTreeMap::new(),
		}
	}

	pub fn get_irq(&self, irq: Irq) -> Irq {
		if let Some(irq) = self.map.get(&irq) {
			*irq
		} else {
			irq
		}
	}

	pub fn override_irq(&mut self, irq_in: Irq, irq_out: Irq) {
		self.map.insert(irq_in, irq_out);
	}
}
