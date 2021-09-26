use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

use crate::uses::*;
use crate::int::apic::LocalApic;
use crate::arch::x64::*;

#[repr(C)]
#[derive(Debug)]
pub struct GsData
{
	// NOTE: these fields have to be first for assmebly code
	pub call_rsp: usize,
	pub call_save_rsp: usize,
	pub proc_id: usize,
	lapic: Option<LocalApic>,
	other_alive: AtomicBool,
}

impl GsData
{
	const fn new() -> Self
	{
		GsData {
			call_rsp: 0x13123,
			call_save_rsp: 0,
			proc_id: 0,
			lapic: None,
			other_alive: AtomicBool::new(false),
		}
	}

	pub fn lapic(&mut self) -> &mut LocalApic {
		self.lapic.as_mut().unwrap()
	}

	pub fn set_lapic(&mut self, lapic: LocalApic) {
		self.lapic = Some(lapic);
	}
}

pub struct GsRef {
	data: *mut GsData,
	intd: IntDisable,
}

impl Deref for GsRef {
	type Target = GsData;

	fn deref(&self) -> &Self::Target {
		unsafe {
			self.data.as_ref().unwrap()
		}
	}
}

impl DerefMut for GsRef {
	fn deref_mut(&mut self) -> &mut Self::Target {
		unsafe {
			self.data.as_mut().unwrap()
		}
	}
}

impl Drop for GsRef {
	fn drop(&mut self) {
		self.other_alive.store(false, Ordering::Release);
	}
}

// panics if another gsref on the same cpu is still alive
pub fn cpud() -> GsRef {
	let intd = IntDisable::new();

	let ptr = gs_addr();

	rprintln!("got addr {:x}", ptr);

	let out = GsRef {
		data: ptr as *mut GsData,
		intd,
	};

	if out.other_alive.swap(true, Ordering::AcqRel) {
		panic!("tried to get multiple gsrefs on the same cpu at the same time");
	}

	out
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct GsDataPtr {
	gsdata_addr: usize,
	temp: usize,
}

pub fn init()
{
	let gsdata_addr = Box::leak(Box::new(GsData::new())) as *mut _ as usize;

	// need this layer of indirection because lea can't be used to get address with gs offset
	// the temp field is used by syscall handler to store rip because there are not enough registers
	let gsptr = GsDataPtr {
		gsdata_addr,
		temp: 0,
	};
	let gs_addr = Box::leak(Box::new(gsptr)) as *mut _ as u64;

	wrmsr(GSBASE_MSR, gs_addr);
	wrmsr(GSBASEK_MSR, gs_addr);
}
