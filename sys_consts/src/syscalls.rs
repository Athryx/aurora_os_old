//! epoch kernel syscall numbers

pub const INFO: u32 = 0;

pub const SPAWN: u32 = 1;

pub const THREAD_NEW: u32 = 2;
pub const THREAD_BLOCK: u32 = 3;

pub const EXIT: u32 = 5;

pub const FUTEX_BLOCK: u32 = 6;
pub const FUTEX_UNBLOCK: u32 = 7;
pub const FUTEX_MOVE: u32 = 8;

pub const SET_PROC_PROPERTIES: u32 = 9;
pub const SET_THREAD_PROPERTIES: u32 = 10;

pub const REALLOC: u32 = 11;

pub const MMIO_MAP: u32 = 12;
pub const MMIO_UNMAP: u32 = 13;
pub const PORT_MAP: u32 = 14;
pub const PORT_UNMAP: u32 = 15;

pub const SALLOC: u32 = 16;
pub const SDEALLOC: u32 = 17;
pub const SMAP: u32 = 18;
pub const SUNMAP: u32 = 19;
pub const SMEM_SIZE: u32 = 20;

pub const MPROTECT: u32 = 21;

pub const REG: u32 = 22;
pub const MSG: u32 = 23;

pub const PRINT_DEBUG: u32 = 24;
