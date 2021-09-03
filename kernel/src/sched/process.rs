use crate::uses::*;
use spin::Mutex;
use bitflags::bitflags;
use core::cell::Cell;
use core::ops::{Index, IndexMut};
use core::sync::atomic::{AtomicUsize, Ordering};
use core::ptr::NonNull;
use alloc::alloc::{Global, Allocator, Layout};
use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};
use crate::mem::{VirtRange, PAGE_SIZE};
use crate::mem::phys_alloc::zm;
use crate::mem::virt_alloc::{VirtMapper, VirtLayout, VirtLayoutElement, PageMappingFlags, FAllocerType, AllocType};
use crate::mem::shared_mem::{SMemMap, SharedMem};
use crate::upriv::PrivLevel;
use crate::util::{LinkedList, AvlTree, IMutex, MemOwner, Futex, UniqueRef, UniqueMut};
use crate::syscall::udata::{UserData, UserArray, UserPageArray};
use super::{ThreadList, TLTreeNode, Namespace, tlist, proc_list, Registers, proc_c, thread_c, int_sched, thread::{Stack, ConnSaveState}};
use super::elf::{ElfParser, Section};
use super::thread::{Thread, ThreadState};
use super::domain::{DomainMap, BlockMode};
use super::connection::{MsgArgs, Connection, ConnectionMap};
use super::sync::FutexMap;

static NEXT_PID: AtomicUsize = AtomicUsize::new (0);

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct SpawnMemPtr
{
	mem: UserPageArray,
	flags: usize,
}

impl SpawnMemPtr
{
	pub fn from_parts (virt_range: VirtRange, flags: SpawnMapFlags) -> Self
	{
		SpawnMemPtr {
			mem: UserPageArray::from_parts (virt_range.as_usize (), virt_range.size () / PAGE_SIZE),
			flags: flags.rwx_bits (),
		}
	}
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct SpawnMemMap
{
	mem: UserPageArray,
	at_addr: usize,
	flags: usize,
}

unsafe impl UserData for SpawnMemMap {}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SpawnStartState
{
	entry: usize,
	mem_arr: UserArray<SpawnMemMap>,
	smem_arr: UserArray<usize>,
}

unsafe impl UserData for SpawnStartState {}

bitflags!
{
	pub struct SpawnMapFlags: usize
	{
		const NONE = 0;
		const READ = 1;
		const WRITE = 1 << 1;
		const EXEC = 1 << 2;
		const NO_COPY = 1 << 3;
		const COPY_ON_WRITE = 1 << 4;
		const MOVE = 1 << 5;
		const PROTECTED = 1 << 6;
		const SPAWN_PTR = 1 << 7;
	}
}

impl SpawnMapFlags
{
	pub fn as_map_flags (&self) -> PageMappingFlags
	{
		let mut out = PageMappingFlags::USER | PageMappingFlags::EXACT_SIZE;
		if self.contains (Self::READ)
		{
			out |= PageMappingFlags::READ;
		}
		if self.contains (Self::WRITE)
		{
			out |= PageMappingFlags::WRITE;
		}
		if self.contains (Self::EXEC)
		{
			out |= PageMappingFlags::EXEC;
		}
		out
	}

	pub fn rwx_bits (&self) -> usize
	{
		get_bits (self.bits (), 0..3)
	}
}

#[derive(Debug)]
pub struct Process
{
	pid: usize,
	name: Arc<Namespace>,
	self_ref: Weak<Self>,

	uid: PrivLevel,

	next_tid: AtomicUsize,
	threads: Mutex<BTreeMap<usize, MemOwner<Thread>>>,

	futex: FutexMap,
	// TODO: change these to use internel futexes to be consistant with FutexMap, which has to use an internal spinlock
	domains: Futex<DomainMap>,
	// NOTE: don't lock proc_list while this is locked
	connections: Futex<ConnectionMap>,
	smem: Futex<SMemMap>,

	pub addr_space: VirtMapper<FAllocerType>,
}

impl Process
{
	// NOTE: must insert into process list before making a thread
	pub fn new (uid: PrivLevel, name: String) -> Arc<Self>
	{
		let pid = NEXT_PID.fetch_add (1, Ordering::Relaxed);
		Arc::new_cyclic (|weak| Self {
			pid,
			name: Namespace::new (name),
			self_ref: weak.clone (),
			uid,
			next_tid: AtomicUsize::new (0),
			threads: Mutex::new (BTreeMap::new ()),
			futex: FutexMap::new (pid),
			domains: Futex::new (DomainMap::new ()),
			connections: Futex::new (ConnectionMap::new (pid)),
			smem: Futex::new (SMemMap::new ()),
			addr_space: VirtMapper::new (&zm),
		})
	}

	// NOTE: this doesn't quite adhere to elf format I think
	// ignores align field, does not enforce that p_vaddr == P_offset % p_align
	// different segments also must not have any overlapping page frames
	pub fn from_elf (elf_data: &[u8], uid: PrivLevel, name: String) -> Result<Arc<Self>, Err>
	{
		let process = Process::new (uid, name);

		let elf = ElfParser::new (elf_data)?;
		let sections = elf.program_headers ();

		let base_flag = if uid.as_cpu_priv ().is_ring3 ()
		{
			PageMappingFlags::USER | PageMappingFlags::READ | PageMappingFlags::EXACT_SIZE
		}
		else
		{
			PageMappingFlags::READ | PageMappingFlags::EXACT_SIZE
		};

		for section in sections.iter ()
		{
			let mut flags = base_flag;

			let sf = section.flags;
			if sf.writable ()
			{
				flags |= PageMappingFlags::WRITE;
			}
			if sf.executable ()
			{
				flags |= PageMappingFlags::EXEC;
			}

			// allocate section backing memory
			// guarenteed to be aligned
			let vrange = section.virt_range;
			let mut mem = zm.allocz (vrange.size ())
				.ok_or_else (|| Err::new ("not enough memory to load executable"))?;

			// copy section data over to memory
			if let Some(data) = section.data
			{
				let memslice = unsafe
				{
					// mem is guarenteed to have enough space
					core::slice::from_raw_parts_mut (mem.as_mut_ptr::<u8> ().add (section.data_offset), data.len ())
				};
				memslice.copy_from_slice (data);
			}

			// construct virtaddr layout
			let v_elem = VirtLayoutElement::from_mem (mem, vrange.size (), flags);
			let vec = vec![v_elem];

			let layout = VirtLayout::from (vec, AllocType::Protected);

			unsafe
			{
				process.addr_space.map_at (layout, vrange)?;
			}
		}

		// in order to avoid a race condition
		// FIXME: this is kind of messy that we have to do this
		let mut plist = proc_list.lock ();
		let pid = process.pid ();
		plist.insert (pid, process.clone ());

		process.new_thread (elf.entry_point (), None).map_err (|err| {
			plist.remove (&pid);
			err
		})?;

		Ok(process)
	}

	pub fn spawn (uid: PrivLevel, name: String, state: SpawnStartState) -> Result<Arc<Self>, SysErr>
	{
		let process = Process::new (uid, name);
		let proc_curr = proc_c ();

		let mem_arr = state.mem_arr.try_fetch ().ok_or (SysErr::InvlPtr)?;
		let mut mem_ptr_arr = Vec::new ();

		for elem in mem_arr
		{
			let flags = SpawnMapFlags::from_bits_truncate (elem.flags);
			let map_flags = flags.as_map_flags ();

			let map_size = elem.mem.byte_len ();

			let velem = if flags.contains (SpawnMapFlags::NO_COPY)
			{
				VirtLayoutElement::new (map_size, map_flags).ok_or (SysErr::OutOfMem)?
			}
			else
			{
				let vrange_from = elem.mem.as_virt_zone ()?;
				let mem = proc_curr.addr_space.copy_to_allocation (vrange_from).ok_or (SysErr::OutOfMem)?;
				VirtLayoutElement::from_mem (mem, map_size, map_flags)
			};

			let atype = if flags.contains (SpawnMapFlags::PROTECTED)
			{
				AllocType::Protected
			}
			else
			{
				AllocType::VirtMem
			};

			let vlayout = VirtLayout::from (vec![velem], atype);

			let mapped_range = if elem.at_addr == 0
			{
				unsafe
				{
					process.addr_space.map (vlayout)?
				}
			}
			else
			{
				let virt_range = VirtRange::try_new_user (elem.at_addr, map_size)?;
				unsafe
				{
					process.addr_space.map_at (vlayout, virt_range)?
				}
			};

			if flags.contains (SpawnMapFlags::SPAWN_PTR)
			{
				mem_ptr_arr.push (SpawnMemPtr::from_parts (mapped_range, flags));
			}
		}

		let mut smem_arr = state.smem_arr.try_fetch ().ok_or (SysErr::InvlPtr)?;

		for smid in smem_arr.iter_mut ()
		{
			let smem = proc_curr.get_smem (*smid).ok_or (SysErr::InvlId)?;
			let new_smid = process.insert_smem (smem);
			*smid = new_smid;
		}

		// offset of smid array after mem_ptr_arr from the start of the memory block they are allocated in
		let mem_ptr_size = mem_ptr_arr.len () * size_of::<SpawnMemPtr> ();
		let smid_size = smem_arr.len () * size_of::<usize> ();
		let total_size = mem_ptr_size + smid_size;

		let smid_offset = align_up (mem_ptr_size, core::mem::align_of::<usize> ());

		let mut mem = zm.alloc (total_size)?;

		let mem_ptr_slice = unsafe { core::slice::from_raw_parts_mut (mem.as_mut_ptr (), mem_ptr_arr.len ()) };
		mem_ptr_slice.copy_from_slice (&mem_ptr_arr[..]);

		let smid_slice = unsafe { core::slice::from_raw_parts_mut ((mem.as_usize () + smid_offset) as *mut _, smem_arr.len ()) };
		smid_slice.copy_from_slice (&smem_arr[..]);

		let velem = VirtLayoutElement::from_mem (mem, total_size, PageMappingFlags::READ | PageMappingFlags::USER);
		let vlayout = VirtLayout::from (vec![velem], AllocType::Protected);
		let mapped_range = unsafe { process.addr_space.map (vlayout)? };
		let addr = mapped_range.as_usize ();

		let mut regs = Registers::from_rip (state.entry);
		regs.rax = addr;
		regs.rbx = mem_ptr_arr.len ();
		regs.rcx = addr + smid_offset;
		regs.rdx = smem_arr.len ();


		let mut plist = proc_list.lock ();
		let pid = process.pid ();
		plist.insert (pid, process.clone ());

		process.new_thread_regs (regs, None).map_err (|err| {
			plist.remove (&pid);
			err
		}).or (Err(SysErr::OutOfMem))?;

		Ok(process)
	}

	pub fn pid (&self) -> usize
	{
		self.pid
	}

	pub fn name (&self) -> &String
	{
		self.name.name ()
	}

	pub fn namespace (&self) -> &Arc<Namespace>
	{
		&self.name
	}

	pub fn uid (&self) -> PrivLevel
	{
		self.uid
	}

	pub fn next_tid (&self) -> usize
	{
		self.next_tid.fetch_add (1, Ordering::Relaxed)
	}

	pub fn futex (&self) -> &FutexMap
	{
		&self.futex
	}

	pub fn domains (&self) -> &Futex<DomainMap>
	{
		&self.domains
	}

	pub fn connections (&self) -> &Futex<ConnectionMap>
	{
		&self.connections
	}

	pub fn insert_connection (&self, connection: Arc<Connection>)
	{
		self.connections.lock ().insert (connection);
	}

	pub fn smem (&self) -> &Futex<SMemMap>
	{
		&self.smem
	}

	pub fn get_smem (&self, smid: usize) -> Option<Arc<SharedMem>>
	{
		Some(self.smem.lock ().get (smid)?.smem ().clone ())
	}

	pub fn insert_smem (&self, smem: Arc<SharedMem>) -> usize
	{
		self.smem.lock ().insert (smem)
	}

	pub fn remove_smem (&self, smid: usize) -> Option<Arc<SharedMem>>
	{
		let entry = self.smem.lock ().remove (smid)?;
		if let Some(vmem) = entry.virt_mem
		{
			unsafe
			{
				self.addr_space.unmap (vmem, entry.smem ().alloc_type ()).unwrap ();
			}
		}
		Some(entry.into_smem ())
	}

	pub fn map_smem (&self, smid: usize) -> Result<VirtRange, SysErr>
	{
		let mut slock = self.smem.lock ();
		let entry = slock.get_mut (smid);
		match entry
		{
			Some(entry) => {
				if entry.virt_mem.is_none ()
				{
					let vlayout = entry.smem ().virt_layout ();
					unsafe
					{
						// do this to use the from trait
						let out = self.addr_space.map (vlayout)?;
						entry.virt_mem = Some(out);
						Ok(out)
					}

				}
				else
				{
					Err(SysErr::InvlOp)
				}
			},
			None => Err(SysErr::InvlId),
		}
	}

	pub fn unmap_smem (&self, smid: usize) -> SysErr
	{
		let mut slock = self.smem.lock ();
		let entry = slock.get_mut (smid);
		match entry
		{
			Some(entry) => {
				if let Some(vmem) = entry.virt_mem
				{
					unsafe
					{
						self.addr_space.unmap (vmem, entry.smem ().alloc_type ()).unwrap ();
					}
					entry.virt_mem = None;
					SysErr::Ok
				}
				else
				{
					SysErr::InvlOp
				}
			},
			None => SysErr::InvlId,
		}
	}

	pub fn get_thread (&self, tid: usize) -> Option<MemOwner<Thread>>
	{
		unsafe
		{
			self.threads.lock ().get (&tid).map (|memown| memown.clone ())
		}
	}

	// returns false if thread with tid is already inserted or tid was not gotten by next tid func
	pub fn insert_thread (&self, thread: MemOwner<Thread>) -> bool
	{
		if thread.tid () >= self.next_tid.load (Ordering::Relaxed)
		{
			return false;
		}

		let mut threads = self.threads.lock ();
		match threads.get (&thread.tid ())
		{
			Some(_) => false,
			None => {
				threads.insert (thread.tid (), thread);
				true
			}
		}
	}

	// sets any threads waiting on this thread to ready to run if thread_list is Some
	// NOTE: acquires the tlproc lock and tlist lock
	// TODO: make process die after last thread removed
	pub fn remove_thread (&self, tid: usize) -> Option<MemOwner<Thread>>
	{
		let out = self.threads.lock ().remove (&tid)?;
		let state = ThreadState::Join(out.tuid ());
		let mut thread_list = tlist.lock ();

		if thread_list.get (state).is_some ()
		{
			// FIXME: ugly
			for tpointer in unsafe { unbound_mut (&mut thread_list[state]).iter () }
			{
				Thread::move_to (tpointer, ThreadState::Ready, &mut thread_list);
			}

			drop (thread_list);

			tlist.dealloc_state (state);
		}

		Some(out)
	}

	// returns tid in ok
	// locks thread list
	// for backwards compatability
	pub fn new_thread (&self, thread_func: usize, name: Option<String>) -> Result<usize, Err>
	{
		self.new_thread_regs (Registers::from_rip (thread_func), name)
	}

	// returns tid in ok
	// locks thread list
	// will override flags, cs, ss, and rsp on regs
	pub fn new_thread_regs (&self, regs: Registers, name: Option<String>) -> Result<usize, Err>
	{
		let tid = self.next_tid ();
		let thread = Thread::new (self.self_ref.clone (), tid, name.unwrap_or_else (|| format! ("{}-thread{}", self.name (), tid)), regs)?;
		if self.insert_thread (unsafe { thread.clone () })
		{
			tlist.lock ()[ThreadState::Ready].push (thread);
			Ok(tid)
		}
		else
		{
			Err(Err::new ("could not insert thread into process thread list"))
		}
	}
}

impl Drop for Process
{
	fn drop (&mut self)
	{
		let mut threads = self.threads.lock ();
		while threads.pop_first ().is_some () {}
		todo! ();
	}
}
