use crate::util::misc::*;

#[derive(Debug, Clone, Copy)]
struct CpuidRet {
	// these are usize so get bits can be easily used
	eax: usize,
	ebx: usize,
	ecx: usize,
	edx: usize,
}

fn cpuid(n: u32) -> CpuidRet {
	let eax: u32;
	let ebx: u32;
	let ecx: u32;
	let edx: u32;
	unsafe {
		asm!("push rbx", 
			 "cpuid",
			 "mov edi, ebx", 
			 "pop rbx",
			 inout("eax") n => eax,
			 out("edi") ebx,
			 out("ecx") ecx,
			 out("edx") edx,
			 options(nomem, nostack));
	}

	CpuidRet {
		eax: eax as usize,
		ebx: ebx as usize,
		ecx: ecx as usize,
		edx: edx as usize,
	}
}

pub fn has_apic() -> bool {
	let vals = cpuid(1);
	get_bits(vals.edx, 9..10) == 1
}
