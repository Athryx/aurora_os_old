use crate::uses::*;
use super::{Sdt, SdtHeader};

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Hpet {
	header: SdtHeader,
}

impl Sdt for Hpet {
	fn header(&self) -> &SdtHeader {
		&self.header
	}
}
