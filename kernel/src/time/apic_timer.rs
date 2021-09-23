use crate::uses::*;
use core::sync::atomic::{AtomicU64, Ordering};
use core::time::Duration;
use crate::arch::x64::cpuid;
use super::NANOSEC_PER_SEC;

const DEFAULT_RESET: Duration = Duration::from_millis(20);

lazy_static! {
	pub static ref apic_timer: ApicTimer = ApicTimer::new(DEFAULT_RESET);
}

pub struct ApicTimer {
	elapsed_time: AtomicU64,
	nano_reset: AtomicU64,
}

impl ApicTimer {
	fn new(reset: Duration) -> Self {
		let out = ApicTimer {
			elapsed_time: AtomicU64::new(0),
			nano_reset: AtomicU64::new(0),
		};
		out.set_reset(reset);
		out
	}

	fn set_reset(&self, reset: Duration) {
		let hz = cpuid::core_clock_freq();
	}
}
