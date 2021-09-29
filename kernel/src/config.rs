//! Config parameters for building kernel

/// Length of the message buffer in pages
pub const MSG_BUF_LEN: usize = 1;

/// Maximum number of logical cpu's supported
pub const MAX_CPUS: usize = 16;

/// How long between interrupts sent by the timer
pub const TIMER_PERIOD: Duration = Duration::from_millis(40);

/// amount of time that elapses before we will switch to a new thread
pub const SCHED_TIME: Duration = Duration::from_millis(10);

// don't tweak the parameters below

use core::sync::atomic::{AtomicBool, Ordering};
use core::time::Duration;
use crate::mem::PAGE_SIZE;
use crate::arch::x64::cpuid;

pub const MSG_BUF_SIZE: usize = MSG_BUF_LEN * PAGE_SIZE;
pub const SCHED_TIME_NANOS: u64 = SCHED_TIME.as_nanos() as u64;

// dynamic config parameters set by kernel

static USE_APIC: AtomicBool = AtomicBool::new(true);

pub fn use_apic() -> bool {
	USE_APIC.load(Ordering::Acquire)
}

pub fn set_use_apic(val: bool) {
	USE_APIC.store(val, Ordering::Release);
}

pub fn init() {
	set_use_apic(cpuid::has_apic());
}
