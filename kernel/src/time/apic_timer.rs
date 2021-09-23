use crate::uses::*;
use core::sync::atomic::{AtomicU64, Ordering};

pub struct ApicTimer {
	elapsed_time: AtomicU64,
	nano_reset: AtomicU64,
}
