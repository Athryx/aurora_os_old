use crate::uses::*;
use alloc::collections::VecDeque;
use crate::cap::{CapObject, CapObjectType};
use crate::sched::Tuid;
use super::Ipcid;

#[derive(Debug)]
pub struct IpcWaitInner {
	tuid: Tuid,
	msg_buf: VirtAddr,
}

#[derive(Debug)]
pub enum IpcWait {
	Send(IpcWaitInner),
	Recv(IpcWaitInner),
	AsyncSend(IpcWaitInner),
	AsyncRecv(IpcWaitInner),
}

#[derive(Debug)]
pub struct Channel {
	waiting: VecDeque<IpcWait>,
}

impl CapObject for Channel {
	fn cap_object_type() -> CapObjectType {
		CapObjectType::Channel
	}

	fn inc_ref(&self) {}
	fn dec_ref(&self) {}
}
