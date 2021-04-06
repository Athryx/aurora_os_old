use core::ops::Deref;
use spin::Mutex;
use crate::arch::x64::*;

pub static gs_data: Mutex<GsData> = Mutex::new (GsData::new ());

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct GsData
{
	pub call_rsp: usize,
	pub call_save_rsp: usize,
}

impl GsData
{
	const fn new () -> Self
	{
		GsData {
			call_rsp: 0,
			call_save_rsp: 0,
		}
	}
}

pub fn init ()
{
	let data_addr = (gs_data.lock ().deref () as *const _) as u64;

	wrmsr (GSBASE_MSR, data_addr);
	wrmsr (GSBASEK_MSR, data_addr);
}
