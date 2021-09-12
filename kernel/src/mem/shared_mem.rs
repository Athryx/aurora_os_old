use alloc::sync::Arc;
use alloc::collections::BTreeMap;

use crate::uses::*;
use crate::util::{Futex, FutexGuard};
use crate::cap::{CapId, CapFlags, Capability, CapObject, CapObjectType, Map};
use super::*;
use super::phys_alloc::{zm, Allocation};
use super::virt_alloc::{AllocType, PageMappingFlags, VirtLayout, VirtLayoutElement};

#[derive(Debug)]
pub struct SharedMem {
	mem: Allocation,
	cap_data: Futex<BTreeMap<CapId, VirtRange>>,
}

impl SharedMem
{
	pub fn new(size: usize, flags: CapFlags) -> Option<Capability<Self>>
	{
		let allocation = zm.alloc(size)?;
		let arc = Arc::new(SharedMem {
			mem: allocation,
			cap_data: Futex::new(BTreeMap::new()),
		});
		Some(Capability::new(arc, flags))
	}
}

impl CapObject for SharedMem {
	fn cap_object_type() -> CapObjectType {
		CapObjectType::SMem
	}

	fn inc_ref(&self) {}
	fn dec_ref(&self) {}
}

impl Map for SharedMem {
	type Lock<'a> = FutexGuard<'a, BTreeMap<CapId, VirtRange>>;

	fn virt_layout(&self, flags: CapFlags) -> VirtLayout {
		let elem = VirtLayoutElement::from_range(
			self.mem.into(),
			PageMappingFlags::from_cap_flags(flags),
		);
		VirtLayout::from(vec![elem], self.alloc_type())
	}

	fn alloc_type(&self) -> AllocType {
		AllocType::Shared
	}

	fn cap_map_data(&self, id: CapId) -> (Option<VirtRange>, Self::Lock<'_>) {
		let lock = self.cap_data.lock();
		let out = lock.get(&id).map(|vr| *vr);
		(out, lock)
	}

	fn set_cap_map_data(&self, id: CapId, data: Option<VirtRange>, mut lock: Self::Lock<'_>) {
		match data {
			Some(virt_range) => lock.insert(id, virt_range),
			None => lock.remove(&id),
		};
	}
}
