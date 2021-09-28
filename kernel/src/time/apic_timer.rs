use crate::uses::*;
use core::sync::atomic::{AtomicU64, Ordering};
use core::time::Duration;
use super::Timer;

const DEFAULT_RESET: Duration = Duration::from_millis(20);

pub static apic_timer: ApicTimer = ApicTimer::new(DEFAULT_RESET);

pub struct ApicTimer {
	elapsed_time: AtomicU64,
	nano_reset: AtomicU64,
}

impl ApicTimer {
	const fn new(reset: Duration) -> Self {
		ApicTimer {
			elapsed_time: AtomicU64::new(0),
			nano_reset: AtomicU64::new(0),
		}
	}
}

impl Timer for ApicTimer {
	fn nsec(&self) -> u64 {
		cpud().lapic().nsec()
	}
}
