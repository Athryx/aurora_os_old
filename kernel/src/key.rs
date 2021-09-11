use crate::uses::*;
use core::sync::atomic::{AtomicUsize, Ordering};
use alloc::sync::Arc;
use crate::cap::{CapObject, CapObjectType, Capability, CapFlags};

static NEXT_KEY: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug)]
pub struct Key(usize);

impl Key {
	pub fn new() -> Capability<Self> {
		let id = NEXT_KEY.fetch_add(1, Ordering::Relaxed);
		Capability::new(Arc::new(Key(id)), CapFlags::empty())
	}

	pub fn id(&self) -> usize {
		self.0
	}
}

impl CapObject for Key {
	fn cap_object_type() -> CapObjectType {
		CapObjectType::Key
	}

	fn inc_ref(&self) {}
	fn dec_ref(&self) {}
}
