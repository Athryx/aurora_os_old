use core::ops::Deref;

use array_const_fn_init::array_const_fn_init;

use crate::uses::*;
use crate::config::MAX_CPUS;
use crate::int::apic::Apic;
use crate::util::IMutex;
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

#[derive(Debug)]
pub struct CpuData {
	pub apic: IMutex<Option<Apic>>,
}

impl CpuData {
	pub const fn new() -> Self {
		CpuData {
			apic: IMutex::new(None),
		}
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
