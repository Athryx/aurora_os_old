//! Config parameters for building kernel

/// Length of the message buffer in pages
pub const MSG_BUF_LEN: usize = 1;

/// Maximum number of logical cpu's supported
pub const MAX_CPUS: usize = 16;

// don't tweak the parameters below

use crate::mem::PAGE_SIZE;
use core::sync::atomic::{AtomicBool, Ordering};

pub const MSG_BUF_SIZE: usize = MSG_BUF_LEN * PAGE_SIZE;

// dynamic config parameters set by kernel

static USE_APIC: AtomicBool = AtomicBool::new(true);

pub fn use_apic() -> bool {
	USE_APIC.load(Ordering::Acquire)
}

pub fn set_use_apic(val: bool) {
	USE_APIC.store(val, Ordering::Release);
}
