pub use core::prelude::v1::*;
pub use alloc::prelude::v1::*;
pub use alloc::{format, vec};
pub use core::mem::size_of;
pub use core::marker::PhantomData;
pub use core::ptr::{self, null, null_mut};
pub use core::cell::RefCell;

pub use sys_consts::SysErr;
pub use lazy_static::lazy_static;
pub use x86_64::{PhysAddr, VirtAddr};
pub use modular_bitfield::prelude::*;

pub use crate::util::misc::*;
pub use crate::util::{Err, Error};
pub use crate::arch::x64::bochs_break;
pub use crate::{eprint, eprintln, init_array, print, println, rprint, rprintln};

pub fn d()
{
	crate::arch::x64::bochs_break();
}
