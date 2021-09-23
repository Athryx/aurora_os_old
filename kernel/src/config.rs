//! Config parameters for building kernel

/// Length of the message buffer in pages
pub const MSG_BUF_LEN: usize = 1;

/// Maximum number of logical cpu's supported
pub const MAX_CPUS: usize = 16;

// don't tweak the parameters below

use crate::mem::PAGE_SIZE;

pub const MSG_BUF_SIZE: usize = MSG_BUF_LEN * PAGE_SIZE;
