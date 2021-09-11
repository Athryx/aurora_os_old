use crate::uses::*;
use crate::make_id_type;
use core::sync::atomic::{AtomicUsize, Ordering};
use alloc::sync::Arc;
use alloc::collections::BTreeMap;
use bitflags::bitflags;
use crate::util::Futex;

bitflags! {
	pub struct CapFlags: usize {
		const READ = 1;
		const WRITE = 1 << 1;
	}
}

pub enum CapObjectType {
	Channel = 0,
	Reply = 1,
	Futex = 2,
	SMem = 3,
	Key = 4,
	Mmio = 5,
	Interrupt = 6,
	Port = 7,
}

impl CapObjectType {
	fn from(n: usize) -> Option<CapObjectType> {
		Some(match n {
			0 => Self::Channel,
			1 => Self::Reply,
			2 => Self::Futex,
			3 => Self::SMem,
			4 => Self::Key,
			5 => Self::Mmio,
			6 => Self::Interrupt,
			7 => Self::Port,
			_ => return None,
		})
	}
}

impl CapObjectType {
	pub fn as_usize(&self) -> usize {
		*self as usize
	}
}

pub trait CapObject {
	fn cap_object_type() -> CapObjectType;
	fn inc_ref(&self);
	fn dec_ref(&self);
}

make_id_type!(CapId);

impl CapId {
	fn flags(self) -> CapFlags {
		CapFlags::from_bits_truncate(self.into())
	}

	fn cap_object_type(self) -> CapObjectType {
		CapObjectType::from(get_bits(self.into(), 2..5)).unwrap()
	}
}

#[derive(Debug)]
pub struct Capability<T: CapObject> {
	object: Arc<T>,
	flags: CapFlags,
	id: CapId,
}

impl<T: CapObject> Capability<T> {
	pub fn new(object: Arc<T>, flags: CapFlags) -> Self {
		object.inc_ref();
		Capability {
			object,
			flags,
			id: CapId::from(0),
		}
	}

	pub fn object(&self) -> &T {
		&self.object
	}

	pub fn flags(&self) -> CapFlags {
		self.flags
	}

	pub fn id(&self) -> CapId {
		self.id
	}

	pub fn arc_clone(&self) -> Arc<T> {
		self.object.clone()
	}

	pub fn and_from_flags(cap: &Self, flags: CapFlags) -> Self {
		let mut out = cap.clone();
		out.flags &= flags;
		out
	}

	pub fn set_base_id(&mut self, id: usize) -> CapId {
		assert!(id < (1 << 59));
		self.id = CapId::from((id << 5) | (T::cap_object_type().as_usize() << 2) | (self.flags.bits()));
		self.id
	}
}

impl<T: CapObject> Clone for Capability<T> {
	fn clone(&self) -> Self {
		self.object.inc_ref();
		Capability {
			object: self.object.clone(),
			flags: self.flags,
			id: CapId::from(0),
		}
	}
}

impl<T: CapObject> Drop for Capability<T> {
	fn drop(&mut self) {
		self.object.dec_ref();
	}
}

#[derive(Debug)]
pub struct CapMap<T: CapObject> {
	data: Futex<BTreeMap<CapId, Capability<T>>>,
	next_id: AtomicUsize,
}

impl<T: CapObject> CapMap<T> {
	pub fn new() -> Self {
		CapMap {
			data: Futex::new(BTreeMap::new()),
			next_id: AtomicUsize::new(0),
		}
	}

	pub fn insert(&self, cap: Capability<T>) -> CapId {
		let id = self.next_id.fetch_add(1, Ordering::Relaxed);
		let id = cap.set_base_id(id);
		self.data.lock().insert(id, cap);
		id
	}

	pub fn remove(&self, id: CapId) -> Option<Capability<T>> {
		self.data.lock().remove(&id)
	}

	pub fn call<F, U>(&self, id: CapId, f: F) -> Option<U>
		where F: FnOnce(&T, CapFlags) -> U
	{
		let lock = self.data.lock();
		let cap = lock.get(&id)?;
		Some(f(&cap.object, cap.flags))
	}

	pub fn clone_cap(&self, id: CapId, flags: CapFlags) -> Option<CapId> {
		let lock = self.data.lock();
		let cap = lock.get(&id)?;
		let new_cap = Capability::and_from_flags(cap, flags);
		drop(lock);
		Some(self.insert(new_cap))
	}
}
