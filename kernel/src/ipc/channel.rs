use crate::uses::*;
use crate::cap::{CapObject, CapObjectType};
use super::Ipcid;

#[derive(Debug)]
pub struct Channel {
	id: Ipcid,
}

impl CapObject for Channel {
	fn cap_object_type() -> CapObjectType {
		CapObjectType::Channel
	}

	fn inc_ref(&self) {}
	fn dec_ref(&self) {}
}
