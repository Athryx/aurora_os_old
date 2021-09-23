pub mod pit;
pub mod apic_timer;
pub use pit::pit as timer;

pub const NANOSEC_PER_SEC: u64 = 1000000000;
