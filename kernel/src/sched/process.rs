use core::cell::Cell;
use core::ops::{Index, IndexMut};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use core::ptr::NonNull;
use alloc::alloc::{Allocator, Global, Layout};
use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};

use spin::Mutex;
use bitflags::bitflags;

use crate::uses::*;
use crate::cap::{CapMap, CapSys, CapObjectType, CapObject};
use crate::key::Key;
use crate::ipc::channel::Channel;
use crate::apic::lapic::Ipi;
use crate::mem::{VirtRange, PAGE_SIZE};
use crate::mem::phys_alloc::zm;
use crate::mem::virt_alloc::{
	AllocType, FAllocerType, PageMappingFlags, VirtLayout, VirtLayoutElement, VirtMapper,
};
use crate::mem::shared_mem::SharedMem;
use crate::upriv::PrivLevel;
use crate::util::{CpuMarker, AvlTree, Futex, IMutex, LinkedList, MemOwner, UniqueMut, UniqueRef};
use crate::syscall::udata::{UserArray, UserData, UserPageArray};
use super::{
	Tid, int_sched, proc_c, proc_list, thread_c, tlist, Registers, TLTreeNode, ThreadList,
};
use super::thread::{ConnSaveState, Stack, ThreadRef, Thread, ThreadState};
use super::elf::{ElfParser, Section};
use super::sync::FutexMap;

static NEXT_PID: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct SpawnMemPtr
{
	mem: UserPageArray,
	flags: usize,
}

impl SpawnMemPtr
{
	pub fn from_parts(virt_range: VirtRange, flags: SpawnMapFlags) -> Self
	{
		SpawnMemPtr {
			mem: UserPageArray::from_parts(virt_range.as_usize(), virt_range.size() / PAGE_SIZE),
			flags: flags.rwx_bits(),
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

bitflags! {
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
	pub fn as_map_flags(&self) -> PageMappingFlags
	{
		let mut out = PageMappingFlags::USER | PageMappingFlags::EXACT_SIZE;
		if self.contains(Self::READ) {
			out |= PageMappingFlags::READ;
		}
		if self.contains(Self::WRITE) {
			out |= PageMappingFlags::WRITE;
		}
		if self.contains(Self::EXEC) {
			out |= PageMappingFlags::EXEC;
		}
		out
	}

	pub fn rwx_bits(&self) -> usize
	{
		get_bits(self.bits(), 0..3)
	}
}

crate::make_id_type!(Pid);

#[derive(Debug)]
pub struct Process
{
	pid: Pid,
	name: String,
	launch_path: String,
	self_ref: Weak<Self>,

	alive: AtomicBool,
	cpus_running: CpuMarker,

	uid: PrivLevel,

	next_tid: AtomicUsize,
	threads: Mutex<BTreeMap<Tid, MemOwner<Thread>>>,

	futex: FutexMap,
	smem: CapMap<SharedMem>,
	channels: CapMap<Channel>,
	keys: CapMap<Key>,

	pub addr_space: VirtMapper<FAllocerType>,
}

impl Process
{
	// NOTE: must insert into process list before making a thread
	pub fn new(uid: PrivLevel, name: String, launch_path: String) -> Arc<Self>
	{
		let pid = NEXT_PID.fetch_add(1, Ordering::Relaxed);
		Arc::new_cyclic(|weak| Self {
			pid: Pid::from(pid),
			name,
			launch_path,
			self_ref: weak.clone(),
			alive: AtomicBool::new(true),
			cpus_running: CpuMarker::new(),
			uid,
			next_tid: AtomicUsize::new(0),
			threads: Mutex::new(BTreeMap::new()),
			futex: FutexMap::new(),
			smem: CapMap::new(),
			channels: CapMap::new(),
			keys: CapMap::new(),
			addr_space: VirtMapper::new(&zm),
		})
	}

	// NOTE: this doesn't quite adhere to elf format I think
	// ignores align field, does not enforce that p_vaddr == P_offset % p_align
	// different segments also must not have any overlapping page frames
	pub fn from_elf(elf_data: &[u8], uid: PrivLevel, name: String, launch_path: String) -> Result<Arc<Self>, Err>
	{
		let process = Process::new(uid, name, launch_path);

		let elf = ElfParser::new(elf_data)?;
		let sections = elf.program_headers();

		let base_flag = if uid.as_cpu_priv().is_ring3() {
			PageMappingFlags::USER | PageMappingFlags::READ | PageMappingFlags::EXACT_SIZE
		} else {
			PageMappingFlags::READ | PageMappingFlags::EXACT_SIZE
		};

		for section in sections.iter() {
			let mut flags = base_flag;

			let sf = section.flags;
			if sf.writable() {
				flags |= PageMappingFlags::WRITE;
			}
			if sf.executable() {
				flags |= PageMappingFlags::EXEC;
			}

			// allocate section backing memory
			// guarenteed to be aligned
			let vrange = section.virt_range;
			let mut mem = zm
				.allocz(vrange.size())
				.ok_or_else(|| Err::new("not enough memory to load executable"))?;

			// copy section data over to memory
			if let Some(data) = section.data {
				let memslice = unsafe {
					// mem is guarenteed to have enough space
					core::slice::from_raw_parts_mut(
						mem.as_mut_ptr::<u8>().add(section.data_offset),
						data.len(),
					)
				};
				memslice.copy_from_slice(data);
			}

			// construct virtaddr layout
			let v_elem = VirtLayoutElement::from_mem(mem, vrange.size(), flags);
			let vec = vec![v_elem];

			let layout = VirtLayout::from(vec, AllocType::Protected);

			unsafe {
				process.addr_space.map_at(layout, vrange)?;
			}
		}

		// in order to avoid a race condition
		// FIXME: this is kind of messy that we have to do this
		let mut plist = proc_list.lock();
		let pid = process.pid();
		plist.insert(pid, process.clone());

		process.new_thread(elf.entry_point(), None).map_err(|err| {
			plist.remove(&pid);
			err
		})?;

		Ok(process)
	}

	pub fn spawn(uid: PrivLevel, name: String, launch_path: String, state: SpawnStartState)
		-> Result<Arc<Self>, SysErr>
	{
		/*let process = Process::new(uid, name, launch_path);
		let proc_curr = proc_c();

		let mem_arr = state.mem_arr.try_fetch().ok_or(SysErr::InvlPtr)?;
		let mut mem_ptr_arr = Vec::new();

		for elem in mem_arr {
			let flags = SpawnMapFlags::from_bits_truncate(elem.flags);
			let map_flags = flags.as_map_flags();

			let map_size = elem.mem.byte_len();

			let velem = if flags.contains(SpawnMapFlags::NO_COPY) {
				VirtLayoutElement::new(map_size, map_flags).ok_or(SysErr::OutOfMem)?
			} else {
				let vrange_from = elem.mem.as_virt_zone()?;
				let mem = proc_curr
					.addr_space
					.copy_to_allocation(vrange_from)
					.ok_or(SysErr::OutOfMem)?;
				VirtLayoutElement::from_mem(mem, map_size, map_flags)
			};

			let atype = if flags.contains(SpawnMapFlags::PROTECTED) {
				AllocType::Protected
			} else {
				AllocType::VirtMem
			};

			let vlayout = VirtLayout::from(vec![velem], atype);

			let mapped_range = if elem.at_addr == 0 {
				unsafe { process.addr_space.map(vlayout)? }
			} else {
				let virt_range = VirtRange::try_new_user(elem.at_addr, map_size)?;
				unsafe { process.addr_space.map_at(vlayout, virt_range)? }
			};

			if flags.contains(SpawnMapFlags::SPAWN_PTR) {
				mem_ptr_arr.push(SpawnMemPtr::from_parts(mapped_range, flags));
			}
		}

		let mut smem_arr = state.smem_arr.try_fetch().ok_or(SysErr::InvlPtr)?;

		for smid in smem_arr.iter_mut() {
			let smem = proc_curr.get_smem(*smid).ok_or(SysErr::InvlId)?;
			let new_smid = process.insert_smem(smem);
			*smid = new_smid;
		}

		// offset of smid array after mem_ptr_arr from the start of the memory block they are allocated in
		let mem_ptr_size = mem_ptr_arr.len() * size_of::<SpawnMemPtr>();
		let smid_size = smem_arr.len() * size_of::<usize>();
		let total_size = mem_ptr_size + smid_size;

		let smid_offset = align_up(mem_ptr_size, core::mem::align_of::<usize>());

		let mut mem = zm.alloc(total_size).ok_or(SysErr::OutOfMem)?;

		let mem_ptr_slice =
			unsafe { core::slice::from_raw_parts_mut(mem.as_mut_ptr(), mem_ptr_arr.len()) };
		mem_ptr_slice.copy_from_slice(&mem_ptr_arr[..]);

		let smid_slice = unsafe {
			core::slice::from_raw_parts_mut(
				(mem.as_usize() + smid_offset) as *mut _,
				smem_arr.len(),
			)
		};
		smid_slice.copy_from_slice(&smem_arr[..]);

		let velem = VirtLayoutElement::from_mem(
			mem,
			total_size,
			PageMappingFlags::READ | PageMappingFlags::USER,
		);
		let vlayout = VirtLayout::from(vec![velem], AllocType::Protected);
		let mapped_range = unsafe { process.addr_space.map(vlayout)? };
		let addr = mapped_range.as_usize();

		let mut regs = Registers::from_rip(state.entry);
		regs.rax = addr;
		regs.rbx = mem_ptr_arr.len();
		regs.rcx = addr + smid_offset;
		regs.rdx = smem_arr.len();

		let mut plist = proc_list.lock();
		let pid = process.pid();
		plist.insert(pid, process.clone());

		process
			.new_thread_regs(regs, None)
			.map_err(|err| {
				plist.remove(&pid);
				err
			})
			.or(Err(SysErr::OutOfMem))?;

		Ok(process)*/
		todo!();
	}

	pub fn pid(&self) -> Pid
	{
		self.pid
	}

	pub fn name(&self) -> &String
	{
		&self.name
	}

	pub fn launch_path(&self) -> &String {
		&self.launch_path
	}

	pub fn is_alive(&self) -> bool {
		self.alive.load(Ordering::Acquire)
	}

	pub fn set_running(&self, running: bool) {
		self.cpus_running.set(running);
	}

	pub fn uid(&self) -> PrivLevel
	{
		self.uid
	}

	pub fn next_tid(&self) -> Tid
	{
		Tid::from(self.next_tid.fetch_add(1, Ordering::Relaxed))
	}

	pub fn futex(&self) -> &FutexMap
	{
		&self.futex
	}

	pub fn smem(&self) -> &CapMap<SharedMem>
	{
		&self.smem
	}

	pub fn channels(&self) -> &CapMap<Channel>
	{
		&self.channels
	}

	pub fn keys(&self) -> &CapMap<Key>
	{
		&self.keys
	}

	pub fn get_capmap(&self, typ: CapObjectType) -> &dyn CapSys {
		match typ {
			CapObjectType::Channel => &self.channels,
			CapObjectType::Futex => &self.futex,
			CapObjectType::SMem => &self.smem,
			CapObjectType::Key => &self.keys,
			CapObjectType::Mmio => todo!(),
			CapObjectType::Interrupt => todo!(),
			CapObjectType::Port => todo!(),
		}
	}

	pub fn get_thread(&self, tid: Tid) -> Option<ThreadRef>
	{
		unsafe { self.threads.lock().get(&tid).map(|memown| ThreadRef::from(memown.clone())) }
	}

	// returns false if thread with tid is already inserted or tid was not gotten by next tid func
	pub fn insert_thread(&self, thread: MemOwner<Thread>) -> bool
	{
		if thread.tid().into() >= self.next_tid.load(Ordering::Relaxed) {
			return false;
		}

		let mut threads = self.threads.lock();
		match threads.get(&thread.tid()) {
			Some(_) => false,
			None => {
				threads.insert(thread.tid(), thread);
				true
			},
		}
	}

	// sets any threads waiting on this thread to ready to run if thread_list is Some
	// NOTE: acquires the tlist lock
	// safety: only call from thread dealloc method
	pub unsafe fn remove_thread(&self, tid: Tid) -> Option<MemOwner<Thread>>
	{
		let out = self.threads.lock().remove(&tid)?;
		let state = ThreadState::Join(out.tuid());
		let mut thread_list = tlist.lock();

		if thread_list.get(state).is_some() {
			// FIXME: ugly
			for tpointer in unbound_mut(&mut thread_list[state]).iter() {
				Thread::move_to(tpointer, ThreadState::Ready, &mut thread_list);
			}

			drop(thread_list);

			tlist.dealloc_state(state);
		}

		Some(out)
	}

	// returns tid in ok
	// locks thread list
	// for backwards compatability
	pub fn new_thread(&self, thread_func: usize, name: Option<String>) -> Result<Tid, Err>
	{
		self.new_thread_regs(Registers::from_rip(thread_func), name)
	}

	// returns tid in ok
	// locks thread list
	// will override flags, cs, ss, and rsp on regs
	pub fn new_thread_regs(&self, regs: Registers, name: Option<String>) -> Result<Tid, Err>
	{
		let tid = self.next_tid();
		let thread = Thread::new(
			self.self_ref.clone(),
			tid,
			name.unwrap_or_else(|| format!("{}-thread{}", self.name(), tid)),
			regs,
		)?;
		if self.insert_thread(unsafe { thread.clone() }) {
			tlist.lock()[ThreadState::Ready].push(thread);
			Ok(tid)
		} else {
			Err(Err::new("could not insert thread into process thread list"))
		}
	}

	// acquires cpu data
	// locks process list
	// locks thread list
	// FIXME: might not always deallocate process if there is an arc on the process that wasn't dropped
	pub fn terminate(&self) {
		proc_list.lock().remove(&self.pid());

		// lock the thread list before setting alive to false so that no threads read alive
		// and then switch to the thread after it is set to false
		let thread_list = tlist.lock();
		self.alive.store(false, Ordering::Release);
		drop(thread_list);

		let mut cpd = cpud();
		let lapic = cpd.lapic();

		let mut self_switch = false;

		// FIXME: i am not sure if the ipi is guarenteed to arrive to cpu before it calls proc_c again,
		// potentially causeing a panic. I think it should though
		for proc_id in self.cpus_running.iter_clear() {
			if proc_id == prid() {
				self_switch = true;
				break;
			}
			lapic.send_ipi(Ipi::process_exit(proc_id));
		}

		// drop cpu data before context switching
		drop(cpd);
		if self_switch {
			int_sched();
		}
	}
}

pub(super) fn ipi_process_exit_handler(_: &mut Registers, _: u64) -> bool {
	// can't use proc_c hear because process may have been dropped
	match thread_c().process() {
		Some(process) => {
			if !process.is_alive() {
				int_sched();
			}
		}
		None => int_sched(),
	}
	false
}
