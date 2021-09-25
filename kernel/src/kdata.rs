use core::ops::{Deref, DerefMut};

use array_const_fn_init::array_const_fn_init;

use crate::uses::*;
use crate::config::MAX_CPUS;
use crate::int::apic::LocalApic;
use crate::int::manager::IrqManager;
use crate::util::{IMutex, IMutexGuard};
use crate::arch::x64::*;

pub static gs_data: IMutex<GsData> = IMutex::new(GsData::new());
//static cpu_data: [CpuData; MAX_CPUS] = [CpuData::new(); MAX_CPUS];
// FIXME: find out a way to use MAX_CPUS instead of putting in 16
static cpu_data: [CpuData; MAX_CPUS] = array_const_fn_init![cpu_data_const; 16];

const fn cpu_data_const(_: usize) -> CpuData {
	CpuData::new()
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct GsData
{
	pub call_rsp: usize,
	pub call_save_rsp: usize,
}

impl GsData
{
	const fn new() -> Self
	{
		GsData {
			call_rsp: 0,
			call_save_rsp: 0,
		}
	}
}

// conveniant reference to cpu data member so you don't have to call a ton of option methods to use the reference
// panics if the referenced field is none when dereferenced
pub struct CpuDataRef<'a, T>(IMutexGuard<'a, Option<T>>);

impl<T> Deref for CpuDataRef<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.0.as_ref().unwrap()
	}
}

impl<T> DerefMut for CpuDataRef<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.0.as_mut().unwrap()
	}
}

#[derive(Debug)]
pub struct CpuData {
	pub lapic: IMutex<Option<LocalApic>>,
}

impl CpuData {
	pub const fn new() -> Self {
		CpuData {
			lapic: IMutex::new(None),
		}
	}

	pub fn lapic(&self) -> CpuDataRef<LocalApic> {
		CpuDataRef(self.lapic.lock())
	}
}

pub fn cpud() -> &'static CpuData {
	cpu_data.get(cpuid::apic_id() as usize).unwrap()
}

pub fn init()
{
	let data_addr = (gs_data.lock().deref() as *const _) as u64;

	wrmsr(GSBASE_MSR, data_addr);
	wrmsr(GSBASEK_MSR, data_addr);
}
