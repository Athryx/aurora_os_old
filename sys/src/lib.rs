//! Aurora kernel system calls
#![no_std]
#![feature(asm)]

use sys_consts::thread;
pub use sys_consts::options::*;
pub use sys_consts::SysErr;

pub const PAGE_SIZE: usize = 4096;
// filler for syscall macro to get right amount of return values
const F: usize = 0;

// must be power of 2 for correct results
const fn align_up(addr: usize, align: usize) -> usize
{
	(addr + align - 1) & !(align - 1)
}

// must be power of 2 for correct results
const fn align_down(addr: usize, align: usize) -> usize
{
	addr & !(align - 1)
}

pub fn thread_new(thread_func: fn() -> ()) -> Result<usize, SysErr>
{
	/*let rip = thread_func as usize;
	let (err, tid) = unsafe { syscall!(THREAD_BLOCK, 0, rip, F) };
	let err = SysErr::new(err).unwrap();

	if err == SysErr::Ok {
		Ok(tid)
	} else {
		Err(err)
	}*/
	unimplemented!();
}

pub enum ThreadState
{
	Yield,
	Destroy,
	Sleep(usize),
	Join(usize),
}

impl ThreadState
{
	fn get_vals(&self) -> (usize, usize)
	{
		let mut val = 0;

		// TODO: put these values in syscall_consts crate
		let reason: usize = match self {
			Self::Yield => thread::YIELD,
			Self::Destroy => thread::DESTROY,
			Self::Sleep(num) => {
				val = *num;
				thread::SLEEP
			},
			Self::Join(num) => {
				val = *num;
				thread::JOIN
			},
		};

		(reason, val)
	}
}

pub fn thread_block(state: ThreadState)
{
	let (reason, val) = state.get_vals();

	/*unsafe {
		syscall!(THREAD_BLOCK, 0, reason, val);
	}*/
}

pub fn futex_block(addr: usize)
{
	unsafe {
		//syscall!(FUTEX_BLOCK, 0, addr);
	}
}

pub fn futex_unblock(addr: usize, n: usize) -> usize
{
	/*let (num, _) = unsafe { syscall!(FUTEX_UNBLOCK, 0, addr, n) };
	num*/
	unimplemented!();
}

pub unsafe fn realloc(
	mem: usize,
	size: usize,
	at_addr: usize,
	options: ReallocOptions,
) -> Result<(usize, usize), SysErr>
{
	unimplemented!();
	/*let (err, mem, len) = syscall!(
		REALLOC,
		options.bits(),
		mem,
		align_up(size, PAGE_SIZE) / PAGE_SIZE,
		at_addr
	);
	let err = SysErr::new(err).unwrap();

	if err == SysErr::Ok {
		Ok((mem, len * PAGE_SIZE))
	} else {
		Err(err)
	}*/
}

pub fn print_debug(bytes: &[u8; 10 * core::mem::size_of::<usize>()], n: u32)
{
	/*let arr: &[usize; 10] = unsafe { core::mem::transmute(bytes) };
	unsafe {
		syscall!(
			PRINT_DEBUG,
			n,
			arr[0],
			arr[1],
			arr[2],
			arr[3],
			arr[4],
			arr[5],
			arr[6],
			arr[7],
			arr[8],
			arr[9]
		);
	}*/
}

// need to use rcx because rbx is reserved by llvm
// FIXME: ugly
#[macro_export]
macro_rules! syscall
{
	($num:expr, $opt:expr) => {{
		asm!("syscall", inout("rax") (($opt as usize) << 32) | ($num as usize) => _);
	}};

	($num:expr, $opt:expr, $a1:expr) => {{
		let o1: usize;
		asm!("push rbx",
			"mov rbx, rcx",
			"syscall",
			"mov rcx, rbx",
			"pop rbx",
			inout("rax") (($opt as usize) << 32) | ($num as usize) => _,
			inout("rcx") $a1 => o1,
			);
		o1
	}};

	($num:expr, $opt:expr, $a1:expr, $a2:expr) => {{
		let o1: usize;
		let o2: usize;
		asm!("push rbx",
			"mov rbx, rcx",
			"syscall",
			"mov rcx, rbx",
			"pop rbx",
			inout("rax") (($opt as usize) << 32) | ($num as usize) => _,
			inout("rcx") $a1 => o1,
			inout("rdx") $a2 => o2,
			);
		(o1, o2)
	}};

	($num:expr, $opt:expr, $a1:expr, $a2:expr, $a3:expr) => {{
		let o1: usize;
		let o2: usize;
		let o3: usize;
		asm!("push rbx",
			"mov rbx, rcx",
			"syscall",
			"mov rcx, rbx",
			"pop rbx",
			inout("rax") (($opt as usize) << 32) | ($num as usize) => _,
			inout("rcx") $a1 => o1,
			inout("rdx") $a2 => o2,
			inout("rsi") $a3 => o3,
			);
		(o1, o2, o3)
	}};

	($num:expr, $opt:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr) => {{
		let o1: usize;
		let o2: usize;
		let o3: usize;
		let o4: usize;
		asm!("push rbx",
			"mov rbx, rcx",
			"syscall",
			"mov rcx, rbx",
			"pop rbx",
			inout("rax") (($opt as usize) << 32) | ($num as usize) => _,
			inout("rcx") $a1 => o1,
			inout("rdx") $a2 => o2,
			inout("rsi") $a3 => o3,
			inout("rdi") $a4 => o4,
			);
		(o1, o2, o3, o4)
	}};

	($num:expr, $opt:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr) => {{
		let o1: usize;
		let o2: usize;
		let o3: usize;
		let o4: usize;
		let o5: usize;
		asm!("push rbx",
			"mov rbx, rcx",
			"syscall",
			"mov rcx, rbx",
			"pop rbx",
			inout("rax") (($opt as usize) << 32) | ($num as usize) => _,
			inout("rcx") $a1 => o1,
			inout("rdx") $a2 => o2,
			inout("rsi") $a3 => o3,
			inout("rdi") $a4 => o4,
			inout("r8") $a5 => o5,
			);
		(o1, o2, o3, o4, o5)
	}};

	($num:expr, $opt:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr, $a6:expr) => {{
		let o1: usize;
		let o2: usize;
		let o3: usize;
		let o4: usize;
		let o5: usize;
		let o6: usize;
		asm!("push rbx",
			"mov rbx, rcx",
			"syscall",
			"mov rcx, rbx",
			"pop rbx",
			inout("rax") (($opt as usize) << 32) | ($num as usize) => _,
			inout("rcx") $a1 => o1,
			inout("rdx") $a2 => o2,
			inout("rsi") $a3 => o3,
			inout("rdi") $a4 => o4,
			inout("r8") $a5 => o5,
			inout("r9") $a6 => o6,
			);
		(o1, o2, o3, o4, o5, o6)
	}};

	($num:expr, $opt:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr, $a6:expr, $a7:expr) => {{
		let o1: usize;
		let o2: usize;
		let o3: usize;
		let o4: usize;
		let o5: usize;
		let o6: usize;
		let o7: usize;
		asm!("push rbx",
			"mov rbx, rcx",
			"syscall",
			"mov rcx, rbx",
			"pop rbx",
			inout("rax") (($opt as usize) << 32) | ($num as usize) => _,
			inout("rcx") $a1 => o1,
			inout("rdx") $a2 => o2,
			inout("rsi") $a3 => o3,
			inout("rdi") $a4 => o4,
			inout("r8") $a5 => o5,
			inout("r9") $a6 => o6,
			inout("r12") $a7 => o7,
			);
		(o1, o2, o3, o4, o5, o6, o7)
	}};

	($num:expr, $opt:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr, $a6:expr, $a7:expr, $a8:expr) => {{
		let o1: usize;
		let o2: usize;
		let o3: usize;
		let o4: usize;
		let o5: usize;
		let o6: usize;
		let o7: usize;
		let o8: usize;
		asm!("push rbx",
			"mov rbx, rcx",
			"syscall",
			"mov rcx, rbx",
			"pop rbx",
			inout("rax") (($opt as usize) << 32) | ($num as usize) => _,
			inout("rcx") $a1 => o1,
			inout("rdx") $a2 => o2,
			inout("rsi") $a3 => o3,
			inout("rdi") $a4 => o4,
			inout("r8") $a5 => o5,
			inout("r9") $a6 => o6,
			inout("r12") $a7 => o7,
			inout("r13") $a8 => o8,
			);
		(o1, o2, o3, o4, o5, o6, o7, o8)
	}};

	($num:expr, $opt:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr, $a6:expr, $a7:expr, $a8:expr, $a9:expr) => {{
		let o1: usize;
		let o2: usize;
		let o3: usize;
		let o4: usize;
		let o5: usize;
		let o6: usize;
		let o7: usize;
		let o8: usize;
		let o9: usize;
		asm!("push rbx",
			"mov rbx, rcx",
			"syscall",
			"mov rcx, rbx",
			"pop rbx",
			inout("rax") (($opt as usize) << 32) | ($num as usize) => _,
			inout("rcx") $a1 => o1,
			inout("rdx") $a2 => o2,
			inout("rsi") $a3 => o3,
			inout("rdi") $a4 => o4,
			inout("r8") $a5 => o5,
			inout("r9") $a6 => o6,
			inout("r12") $a7 => o7,
			inout("r13") $a8 => o8,
			inout("r14") $a9 => o9,
			);
		(o1, o2, o3, o4, o5, o6, o7, o8, o9)
	}};

	($num:expr, $opt:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr, $a6:expr, $a7:expr, $a8:expr, $a9:expr, $a10:expr) => {{
		let o1: usize;
		let o2: usize;
		let o3: usize;
		let o4: usize;
		let o5: usize;
		let o6: usize;
		let o7: usize;
		let o8: usize;
		let o9: usize;
		let o10: usize;
		asm!("push rbx",
			"mov rbx, rcx",
			"syscall",
			"mov rcx, rbx",
			"pop rbx",
			inout("rax") (($opt as usize) << 32) | ($num as usize) => _,
			inout("rcx") $a1 => o1,
			inout("rdx") $a2 => o2,
			inout("rsi") $a3 => o3,
			inout("rdi") $a4 => o4,
			inout("r8") $a5 => o5,
			inout("r9") $a6 => o6,
			inout("r12") $a7 => o7,
			inout("r13") $a8 => o8,
			inout("r14") $a9 => o9,
			inout("r15") $a10 => o10,
			);
		(o1, o2, o3, o4, o5, o6, o7, o8, o9, o10)
	}};
}
