use crate::uses::*;
use core::sync::atomic::{AtomicUsize, Ordering};
use core::ops::DerefMut;
use alloc::sync::Arc;
use alloc::collections::BTreeMap;
use bitflags::bitflags;
use crate::util::FutexGuard;
use crate::make_id_type;
use crate::mem::{PAGE_SIZE, VirtRange};
use crate::sched::proc_c;
use crate::mem::virt_alloc::{VirtLayout, AllocType};
use crate::util::Futex;

pub mod sys;

bitflags! {
	pub struct CapFlags: usize {
		const READ = 1;
		const WRITE = 1 << 1;
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapObjectType {
	Channel = 0,
	Futex = 1,
	SMem = 2,
	Key = 3,
	Mmio = 4,
	Interrupt = 5,
	Port = 6,
}

impl CapObjectType {
	fn from(n: usize) -> Option<CapObjectType> {
		Some(match n {
			0 => Self::Channel,
			1 => Self::Futex,
			2 => Self::SMem,
			3 => Self::Key,
			4 => Self::Mmio,
			5 => Self::Interrupt,
			6 => Self::Port,
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

pub trait Map: CapObject {
	type Lock<'a>;

	fn virt_layout(&self, flags: CapFlags) -> VirtLayout;
	fn alloc_type(&self) -> AllocType;
	fn cap_map_data(&self, id: CapId) -> (Option<VirtRange>, Self::Lock<'_>);
	fn set_cap_map_data(&self, id: CapId, data: Option<VirtRange>, lock: Self::Lock<'_>);

	fn map(&self, id: CapId, at_addr: Option<usize>) -> Result<VirtRange, SysErr> {
		let (layout, lock) = self.cap_map_data(id);

		match layout {
			Some(_) => Err(SysErr::InvlOp),
			None => {
				let vlayout = self.virt_layout(id.flags());
				let virt_range = if let Some(at_addr) = at_addr {
					if !page_aligned(at_addr) {
						return Err(SysErr::InvlAlign);
					}
					let vaddr = VirtAddr::try_new(at_addr as u64).or(Err(SysErr::InvlVirtAddr))?;
					let vrange = VirtRange::new(vaddr, vlayout.size());

					unsafe {
						proc_c().addr_space.map_at(vlayout, vrange)?
					}
				} else {
					unsafe {
						proc_c().addr_space.map(vlayout)?
					}
				};

				self.set_cap_map_data(id, Some(virt_range), lock);
				Ok(virt_range)
			},
		}
	}

	fn unmap(&self, id: CapId) -> Result<(), SysErr> {
		let (layout, lock) = self.cap_map_data(id);

		match layout {
			Some(layout) => {
				unsafe {
					proc_c().addr_space.unmap(layout, self.alloc_type()).unwrap();
				}
				self.set_cap_map_data(id, None, lock);
				Ok(())
			},
			None => Err(SysErr::InvlOp),
		}
	}
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

	pub fn and_from_flags(cap: &Self, flags: CapFlags) -> Self {
		let mut out = cap.clone();
		out.flags &= flags;
		out
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

// Syscalls on capabilities go in here
pub trait CapSys {
	fn destroy(&self, id: CapId) -> bool;
	fn clone_cap(&self, id: CapId, flags: CapFlags) -> Option<CapId>;
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

	pub fn insert(&self, mut cap: Capability<T>) -> CapId {
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

	pub fn clone_from(&self, id: CapId) -> Option<Capability<T>> {
		let lock = self.data.lock();
		Some(lock.get(&id)?.clone())
	}
}

impl<T: Map> CapMap<T> {
	fn map(&self, id: CapId, at_addr: Option<usize>) -> Result<VirtRange, SysErr> {
		let lock = self.data.lock();
		match lock.get(&id) {
			Some(cap) => cap.object().map(id, at_addr),
			None => Err(SysErr::InvlId),
		}
	}

	fn unmap(&self, id: CapId) -> Result<(), SysErr> {
		let lock = self.data.lock();
		match lock.get(&id) {
			Some(cap) => cap.object().unmap(id),
			None => Err(SysErr::InvlId),
		}
	}
}

impl<T: CapObject> CapSys for CapMap<T> {
	fn destroy(&self, id: CapId) -> bool {
		self.remove(id).is_some()
	}

	fn clone_cap(&self, id: CapId, flags: CapFlags) -> Option<CapId> {
		let lock = self.data.lock();
		let cap = lock.get(&id)?;
		let new_cap = Capability::and_from_flags(cap, flags);
		drop(lock);
		Some(self.insert(new_cap))
	}
}
