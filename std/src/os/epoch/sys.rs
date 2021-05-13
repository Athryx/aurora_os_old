//! Epoch kernel system calls

/*pub fn thread_new (thread_func: fn() -> ()) -> Result<usize, 
{
}*/

// Raw interface to kernel's syscall abi
fn syscall_raw (
	num: u32,
	options: u32,
	a1: usize,
	a2: usize,
	a3: usize,
	a4: usize,
	a5: usize,
	a6: usize,
) -> (usize, usize)
{
	let out1;
	let out2;

	unsafe
	{
		asm!(
			"syscall",
			inout("rax") a6 => out1,
			in("rbx") a2,
			inout("rcx") 0 => _,
			inout("rdx") a1 => out2,
			in("rsi") num as usize | ((options as usize) << 32),
			in("rdi") a5,
			in("r8") a3,
			in("r9") a4,
			inout("r10") 0 => _,
			inout("r11") 0 => _,
		);
	}

	(out1, out2)
}

// Raw interface to kernel's syscall abi, with extended arguments
fn syscall_raw_ext (
	num: u32,
	options: u32,
	a1: usize,
	a2: usize,
	a3: usize,
	a4: usize,
	a5: usize,
	a6: usize,
	a7: usize,
	a8: usize,
	a9: usize,
	a10: usize
) -> (usize, usize)
{
	let out1;
	let out2;

	unsafe
	{
		asm!(
			"syscall",
			inout("rax") a6 => out1,
			in("rbx") a2,
			inout("rcx") 0 => _,
			inout("rdx") a1 => out2,
			in("rsi") num as usize | ((options as usize) << 32),
			in("rdi") a5,
			in("r8") a3,
			in("r9") a4,
			inout("r10") 0 => _,
			inout("r11") 0 => _,
			in("r12") a7,
			in("r13") a8,
			in("r14") a9,
			in("r15") a10,
		);
	}

	(out1, out2)
}
