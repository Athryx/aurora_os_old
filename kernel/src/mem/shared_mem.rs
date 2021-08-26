use crate::uses::*;
use bitflags::bitflags;
use core::sync::atomic::{AtomicUsize, Ordering};
use alloc::sync::Arc;
use alloc::collections::BTreeMap;
use super::*;
use super::phys_alloc::{zm, Allocation};

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
}

#[derive(Debug)]
pub struct SMemMap
{
	data: BTreeMap<usize, Arc<SharedMem>>,
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
		self.data.insert (id, smem);
		id
	}

	pub fn get (&self, id: usize) -> Option<&Arc<SharedMem>>
	{
		self.data.get (&id)
	}

	pub fn remove (&mut self, id: usize) -> Option<Arc<SharedMem>>
	{
		self.data.remove (&id)
	}
}
