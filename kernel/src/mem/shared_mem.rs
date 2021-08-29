use crate::uses::*;
use bitflags::bitflags;
use core::sync::atomic::{AtomicUsize, Ordering};
use alloc::sync::Arc;
use alloc::collections::BTreeMap;
use super::*;
use super::phys_alloc::{zm, Allocation};
use super::virt_alloc::{PageMappingFlags, VirtLayoutElement, VirtLayout, AllocType};

bitflags!
{
	pub struct SMemFlags: u8
	{
		const NONE =		0;
		const READ =		1;
		const WRITE =		1 << 1;
		const EXEC =		1 << 2;
	}
}

impl SMemFlags
{
	fn exists (&self) -> bool
	{
		self.intersects (SMemFlags::READ | SMemFlags::WRITE | SMemFlags::EXEC)
	}
}

static next_smid: AtomicUsize = AtomicUsize::new (0);

#[derive(Debug)]
pub struct SharedMem
{
	mem: Allocation,
	flags: SMemFlags,
	// this id is not used in any process to reference this shared memory, it is used for scheduler purposes to wait on shared futexes
	id: usize,
}

impl SharedMem
{
	pub fn new (size: usize, flags: SMemFlags) -> Option<Arc<Self>>
	{
		let allocation = zm.alloc (size)?;
		Some(Arc::new (SharedMem {
			mem: allocation,
			flags,
			id: next_smid.fetch_add (1, Ordering::Relaxed),
		}))
	}

	pub fn id (&self) -> usize
	{
		self.id
	}

	pub fn alloc_type (&self) -> AllocType
	{
		AllocType::Shared(self.id)
	}

	// returns a virtual layout that can be mapped by the virtual memory mapper
	pub fn virt_layout (&self) -> VirtLayout
	{
		let elem = VirtLayoutElement::from_range (self.mem.into (), PageMappingFlags::from_shared_flags (self.flags));
		VirtLayout::from (vec![elem], self.alloc_type ())
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SMemAddr
{
	smid: usize,
	offset: usize,
}

impl SMemAddr
{
	pub fn new (smid: usize, offset: usize) -> Self
	{
		SMemAddr {
			smid,
			offset,
		}
	}

	pub fn smid (&self) -> usize
	{
		self.smid
	}

	pub fn offset (&self) -> usize
	{
		self.offset
	}
}

impl Default for SMemAddr
{
	fn default () -> Self
	{
		Self::new (0, 0)
	}
}

#[derive(Debug)]
pub struct SMemMapEntry
{
	smem: Arc<SharedMem>,
	pub virt_mem: Option<VirtRange>,
}

impl SMemMapEntry
{
	pub fn smem (&self) -> &Arc<SharedMem>
	{
		&self.smem
	}

	pub fn into_smem (self) -> Arc<SharedMem>
	{
		self.smem
	}
}

#[derive(Debug)]
pub struct SMemMap
{
	data: BTreeMap<usize, SMemMapEntry>,
	next_id: usize,
}

impl SMemMap
{
	pub fn new () -> Self
	{
		SMemMap {
			data: BTreeMap::new (),
			next_id: 0,
		}
	}

	pub fn insert (&mut self, smem: Arc<SharedMem>) -> usize
	{
		let id = self.next_id;
		self.next_id += 1;
		let entry = SMemMapEntry {
			smem,
			virt_mem: None,
		};
		self.data.insert (id, entry);
		id
	}

	pub fn get (&self, id: usize) -> Option<&SMemMapEntry>
	{
		self.data.get (&id)
	}

	pub fn get_mut (&mut self, id: usize) -> Option<&mut SMemMapEntry>
	{
		self.data.get_mut (&id)
	}

	pub fn remove (&mut self, id: usize) -> Option<SMemMapEntry>
	{
		self.data.remove (&id)
	}
}
