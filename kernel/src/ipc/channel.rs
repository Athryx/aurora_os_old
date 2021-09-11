use crate::uses::*;
use crate::cap::{CapObject, CapObjectType};

pub struct Channel {
}

impl CapObject for Channel {
	fn cap_object_type() -> CapObjectType {
		CapObjectType::Channel
	}

	fn inc_ref(&self, n: i8) {}
}
