pub mod pit;
pub mod apic_timer;

pub use pit::pit;
pub use apic_timer::apic_timer;
pub use core::time::Duration;

use crate::config;

pub trait Timer {
	fn nsec(&self) -> u64;

	fn nsec_no_latch(&self) -> u64 {
		self.nsec()
	}

	fn duration(&self) -> Duration {
		Duration::from_nanos(self.nsec())
	}

	fn duration_no_latch(&self) -> Duration {
		Duration::from_nanos(self.nsec_no_latch())
	}
}

pub fn timer() -> &'static dyn Timer {
	if config::use_apic() {
		&apic_timer
	} else {
		&pit
	}
}

pub const NANOSEC_PER_SEC: u64 = 1000000000;
